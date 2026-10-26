import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MAX_CURVE_DEGREES, arcCurve, arcLength, curvedShape } from './curves.js'

// The centre of the arc: on the perpendicular through the middle of the chord, on the side away from the bend
const centre = (length, radius, angle, side) => ({
  x: length / 2,
  y: side === 'left' ? (radius * Math.cos((angle * Math.PI) / 360)) : -(radius * Math.cos((angle * Math.PI) / 360)),
})

test('the ends of an arc are its chord: 2R sin(angle / 2) apart', () => {
  const curve = arcCurve(300, 90, 'left')
  assert.equal(curve.length, 424.26)
  assert.equal(arcLength(300, 90).toFixed(2), '471.24')
})

test('a curve of a radius and angle lies on that circle', () => {
  for (const [radius, angle, side] of [[300, 90, 'left'], [150, 45, 'right'], [400, 15, 'left']]) {
    const curve = arcCurve(radius, angle, side)
    const shape = curvedShape(curve.length, curve.handles)
    const middle = centre(curve.length, radius, angle, side)
    for (const point of shape.paths[0]) {
      const distance = Math.hypot(point.x - middle.x, point.y - middle.y)
      assert.ok(Math.abs(distance - radius) / radius < 0.005, `${radius} mm ${angle} deg: ${distance} mm from the centre`)
    }
  }
})

test('the curve bends the way it is asked to', () => {
  const left = curvedShape(arcCurve(300, 90, 'left').length, arcCurve(300, 90, 'left').handles)
  const right = curvedShape(arcCurve(300, 90, 'right').length, arcCurve(300, 90, 'right').handles)
  const top = left.paths[0][Math.floor(left.paths[0].length / 2)]
  const bottom = right.paths[0][Math.floor(right.paths[0].length / 2)]
  assert.ok(top.y < 0, 'left bends toward negative y')
  assert.ok(bottom.y > 0, 'right bends toward positive y')
})

test('a curve needs a radius above zero and an angle from 1 to 90 degrees', () => {
  assert.throws(() => arcCurve(0, 90, 'left'))
  assert.throws(() => arcCurve(300, 0, 'left'))
  assert.throws(() => arcCurve(300, MAX_CURVE_DEGREES + 1, 'left'))
})
