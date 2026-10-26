// Curved flex track. The track is a cubic Bezier from the origin to (length, 0). Two handles set the curve:
// the first lies along the start, the second along the end, so the track leaves and arrives at the angles the
// handles give. The track passes through its two ends only. Coordinates are the piece's own, in mm

const CURVE_STEPS = 24

const rad = (degrees) => (degrees * Math.PI) / 180
const deg = (radians) => (radians * 180) / Math.PI
const normalize = (degrees) => ((degrees % 360) + 360) % 360

function bezierPoint(p0, p1, p2, p3, t) {
  const u = 1 - t
  const a = u * u * u
  const b = 3 * u * u * t
  const c = 3 * u * t * t
  const d = t * t * t
  return {
    x: a * p0.x + b * p1.x + c * p2.x + d * p3.x,
    y: a * p0.y + b * p1.y + c * p2.y + d * p3.y,
  }
}

// Shape of flex track of `length` set by its two handles
export function curvedShape(length, handles) {
  const p0 = { x: 0, y: 0 }
  const p1 = handles[0]
  const p2 = handles[1]
  const p3 = { x: length, y: 0 }
  const path = []
  for (let step = 0; step <= CURVE_STEPS; step += 1) path.push(bezierPoint(p0, p1, p2, p3, step / CURVE_STEPS))
  // Directions the track leaves the start and arrives at the end. A handle on its end point falls back to the other handle
  const enter = { x: p1.x - p0.x || p2.x - p0.x, y: p1.y - p0.y || p2.y - p0.y }
  const leave = { x: p3.x - p2.x || p3.x - p1.x, y: p3.y - p2.y || p3.y - p1.y }
  return {
    endpoints: [
      { x: 0, y: 0, heading: normalize(deg(Math.atan2(enter.y, enter.x)) + 180) },
      { x: length, y: 0, heading: normalize(deg(Math.atan2(leave.y, leave.x))) },
    ],
    paths: [path],
  }
}

// Sum of turning squared over length along a sampled path. Lower is a fairer, more even curve
export function bendingEnergy(path) {
  let energy = 0
  for (let i = 1; i < path.length - 1; i += 1) {
    const a = path[i - 1]
    const b = path[i]
    const c = path[i + 1]
    const v1 = { x: b.x - a.x, y: b.y - a.y }
    const v2 = { x: c.x - b.x, y: c.y - b.y }
    const l1 = Math.hypot(v1.x, v1.y)
    const l2 = Math.hypot(v2.x, v2.y)
    if (l1 === 0 || l2 === 0) continue
    const turn = Math.atan2(v1.x * v2.y - v1.y * v2.x, v1.x * v2.x + v1.y * v2.y)
    energy += (turn * turn) / ((l1 + l2) / 2)
  }
  return energy
}

// The two handles that make the curve fairest, keeping both ends' angles and the length. Each handle stays on
// its end's line, so only how far along that line it sits changes
export function smoothHandles(shape, length) {
  const startAngle = rad(normalize(shape.endpoints[0].heading - 180))
  const endAngle = rad(shape.endpoints[1].heading)
  const build = (r1, r2) => [
    { x: r1 * Math.cos(startAngle), y: r1 * Math.sin(startAngle) },
    { x: length - r2 * Math.cos(endAngle), y: -r2 * Math.sin(endAngle) },
  ]
  const cost = (r1, r2) => bendingEnergy(curvedShape(length, build(r1, r2)).paths[0])

  // Coarse search over how far each handle sits along its line, then refine around the best
  let best = { r1: length / 3, r2: length / 3, value: Infinity }
  const steps = 24
  for (let i = 1; i <= steps; i += 1) {
    for (let j = 1; j <= steps; j += 1) {
      const r1 = (i / steps) * length
      const r2 = (j / steps) * length
      const value = cost(r1, r2)
      if (value < best.value) best = { r1, r2, value }
    }
  }
  let spread = length / steps
  for (let round = 0; round < 6; round += 1) {
    const centre = best
    for (let i = -4; i <= 4; i += 1) {
      for (let j = -4; j <= 4; j += 1) {
        const r1 = Math.max(0.01 * length, centre.r1 + (i * spread) / 4)
        const r2 = Math.max(0.01 * length, centre.r2 + (j * spread) / 4)
        const value = cost(r1, r2)
        if (value < best.value) best = { r1, r2, value }
      }
    }
    spread /= 4
  }
  return build(best.r1, best.r2)
}

// A circular arc of `radius` that turns through `angleDegrees`, as the flex track that matches it. The ends are the
// arc's chord, so the length is 2R sin(θ/2). The handles lie along the tangents at the ends, at (4/3) tan(θ/4) R, which
// is the cubic Bezier that reproduces a circle to about 0.03% up to a quarter turn. A longer bend is two curves, since one
// cubic is about 1.3% off over a half turn. 'left' bends toward negative y, which is left of the start direction
export const MAX_CURVE_DEGREES = 90

export function arcCurve(radius, angleDegrees, side) {
  if (!(radius > 0) || !(angleDegrees > 0 && angleDegrees <= MAX_CURVE_DEGREES)) {
    throw new Error(`A curve needs a radius above 0 and an angle from 1 to ${MAX_CURVE_DEGREES} degrees`)
  }
  const theta = rad(angleDegrees)
  const length = 2 * radius * Math.sin(theta / 2)
  const handle = (4 / 3) * Math.tan(theta / 4) * radius
  const dx = handle * Math.cos(theta / 2)
  const dy = handle * Math.sin(theta / 2) * (side === 'left' ? -1 : 1)
  const round = (value) => Math.round(value * 100) / 100
  return {
    length: round(length),
    handles: [
      { x: round(dx), y: round(dy) },
      { x: round(length - dx), y: round(dy) },
    ],
  }
}

// The length of the arc itself, which is longer than the ends are apart
export function arcLength(radius, angleDegrees) {
  return radius * rad(angleDegrees)
}

// The arc a flex track follows, read back from its length and handles: its radius, the angle it turns through, and the
// side it bends to. A track with no bend, or a shape that is not an arc, reads as null. Handles dragged by hand give an
// arc that fits the shape as closely as its two tangents allow
export function curveOf(length, handles) {
  if (!Array.isArray(handles) || handles.length !== 2 || !(length > 0)) return null
  const [p1, p2] = handles
  const start = Math.atan2(p1.y, p1.x)
  const arrive = Math.atan2(-p2.y, length - p2.x)
  const turn = arrive - start
  const angle = Math.abs(deg(turn))
  if (!(angle > 0.05) || angle >= 180) return null
  const radius = length / (2 * Math.sin(Math.abs(turn) / 2))
  if (!Number.isFinite(radius)) return null
  return { radius, angle, side: turn > 0 ? 'left' : 'right' }
}
