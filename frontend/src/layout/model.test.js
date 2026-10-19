import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  snap,
  snapPoint,
  distanceToSegment,
  hitTrack,
  hitItem,
  nextName,
  nextFeedbackNumber,
  signalColor,
  fromServer,
  newItem,
  createBody,
  snapshotOf,
  diffItems,
} from './model.js'

test('snap rounds to the 20 unit grid', () => {
  assert.equal(snap(0), 0)
  assert.equal(snap(9), 0)
  assert.equal(snap(11), 20)
  assert.equal(snap(-11), -20)
  assert.deepEqual(snapPoint({ x: 31, y: 49 }), { x: 40, y: 40 })
})

test('distanceToSegment handles interior, endpoint, and degenerate segments', () => {
  const a = { x: 0, y: 0 }
  const b = { x: 100, y: 0 }
  assert.equal(distanceToSegment({ x: 50, y: 5 }, a, b), 5)
  assert.equal(distanceToSegment({ x: 120, y: 0 }, a, b), 20)
  assert.equal(distanceToSegment({ x: 3, y: 4 }, a, a), 5)
})

test('hitTrack finds the nearest track line and misses far points', () => {
  const tracks = [
    { id: 't1', x1: 0, y1: 0, x2: 100, y2: 0 },
    { id: 't2', x1: 0, y1: 100, x2: 100, y2: 100 },
  ]
  assert.equal(hitTrack(tracks, { x: 50, y: 3 }).id, 't1')
  assert.equal(hitTrack(tracks, { x: 50, y: 98 }).id, 't2')
  assert.equal(hitTrack(tracks, { x: 50, y: 50 }), null)
})

test('hitItem returns the top-most item near the point', () => {
  const items = [
    { key: 'a', x: 100, y: 100 },
    { key: 'b', x: 104, y: 100 },
  ]
  assert.equal(hitItem(items, { x: 102, y: 100 }).key, 'b')
  assert.equal(hitItem(items, { x: 300, y: 300 }), null)
})

test('nextName counts items of the same kind', () => {
  const items = [
    { kind: 'block', name: 'Block 1' },
    { kind: 'signal', name: 'Signal 1' },
  ]
  assert.equal(nextName('block', items), 'Block 2')
  assert.equal(nextName('feedback', items), 'Feedback 1')
})

test('nextFeedbackNumber continues from the highest number', () => {
  assert.equal(nextFeedbackNumber([]), 1)
  assert.equal(
    nextFeedbackNumber([
      { kind: 'feedback', feedback_number: 4 },
      { kind: 'block', name: 'x' },
    ]),
    5
  )
})

test('signalColor maps aspects to colours and defaults to stop', () => {
  assert.equal(signalColor('go'), '#22c55e')
  assert.equal(signalColor('caution'), '#eab308')
  assert.equal(signalColor('stop'), '#ef4444')
  assert.equal(signalColor('anything else'), '#ef4444')
})

test('createBody includes the kind-specific fields', () => {
  const signal = newItem('signal', 1, { name: 'S', x: 20, y: 40, signal_type: 'warning' })
  assert.deepEqual(createBody(signal, 7), {
    layout_id: 7,
    name: 'S',
    x: 20,
    y: 40,
    signal_type: 'warning',
  })
  const feedback = newItem('feedback', 1, { name: 'F', x: 0, y: 0, feedback_number: 3 })
  assert.equal(createBody(feedback, 7).feedback_number, 3)
})

test('diffItems creates new items, updates moved ones, and deletes removed ones', () => {
  const saved = [
    { id: 1, kind: 'block', name: 'A', x: 0, y: 0 },
    { id: 2, kind: 'block', name: 'B', x: 20, y: 20 },
  ].map((row) => fromServer('block', row))
  const snapshot = snapshotOf(saved)

  const items = [
    { ...saved[0], x: 40 }, // moved
    newItem('block', 1, { name: 'C', x: 60, y: 60, layoutId: 9 }), // new
  ]
  items[1].layoutId = 9

  const ops = diffItems(snapshot, items)
  assert.deepEqual(ops, [
    { op: 'update', kind: 'block', id: 1, body: { name: 'A', x: 40, y: 0 } },
    {
      op: 'create',
      kind: 'block',
      key: 'new:block:1',
      body: { layout_id: 9, name: 'C', x: 60, y: 60 },
    },
    { op: 'delete', kind: 'block', id: 2 },
  ])
})

test('diffItems produces nothing when nothing changed', () => {
  const saved = [fromServer('signal', { id: 5, name: 'S', x: 0, y: 0, signal_type: 'main' })]
  assert.deepEqual(diffItems(snapshotOf(saved), saved), [])
})
