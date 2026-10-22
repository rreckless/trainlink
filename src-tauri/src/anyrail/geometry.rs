//! Geometry for the AnyRail import: angles, placements, and the shapes of turnouts, slips, three-ways, and arcs.
//! Angles are in degrees. A piece's ends are (point, heading) pairs in its own frame, where the heading is the
//! direction of travel leaving the piece through that end. Flip mirrors a piece across its x axis first.

use serde_json::Value;

pub type Point = (f64, f64);

pub fn angle(start: Point, end: Point) -> f64 {
    (end.1 - start.1).atan2(end.0 - start.0).to_degrees()
}

pub fn normalize(degrees: f64) -> f64 {
    ((degrees % 360.0) + 360.0) % 360.0
}

/// Angle between two undirected lines, 0 to 90 degrees
pub fn line_difference(a: f64, b: f64) -> f64 {
    let d = ((a - b) % 180.0 + 180.0) % 180.0;
    d.min(180.0 - d)
}

pub fn rotate(point: Point, degrees: f64) -> Point {
    let (sin, cos) = degrees.to_radians().sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

fn mirror(point: Point, flip: bool) -> Point {
    if flip {
        (point.0, -point.1)
    } else {
        point
    }
}

pub fn world(point: Point, origin: Point, rotation: f64, flip: bool) -> Point {
    let (x, y) = rotate(mirror(point, flip), rotation);
    (origin.0 + x, origin.1 + y)
}

/// Origin and rotation that put end `index` of an unflipped piece at `point`, heading `required_heading`
pub fn align(ends: &[(Point, f64)], index: usize, point: Point, required_heading: f64) -> (Point, f64) {
    let (local_point, local_heading) = ends[index];
    let rotation = required_heading - local_heading;
    let (rx, ry) = rotate(local_point, rotation);
    ((point.0 - rx, point.1 - ry), rotation)
}

/// Toe, straight, and diverging ends of a turnout in its own frame. Its diverging side is +y for right-hand
pub fn turnout_ends(piece: &Value) -> [(Point, f64); 3] {
    let length = piece["length"].as_f64().unwrap_or_default();
    let side = if piece["hand"].as_str() == Some("right") { 1.0 } else { -1.0 };
    let (diverge, sweep) = match piece.get("diverge_geometry").filter(|value| value.is_object()) {
        Some(geometry) => {
            let radius = geometry["radius"].as_f64().unwrap_or_default();
            let total = geometry["length"].as_f64().unwrap_or_default();
            let toe = geometry["toe_straight"].as_f64().unwrap_or_default();
            let sweep = ((total - toe) / radius).asin();
            ((total, side * radius * (1.0 - sweep.cos())), sweep)
        }
        None => {
            let radius = piece["radius"].as_f64().unwrap_or_default();
            let sweep = (length / radius).asin();
            ((length, side * radius * (1.0 - sweep.cos())), sweep)
        }
    };
    [
        ((0.0, 0.0), 180.0),
        ((length, 0.0), 0.0),
        (diverge, side * sweep.to_degrees()),
    ]
}

/// Toe, straight end, right branch end, and left branch end of a three-way, in its own frame
pub fn three_way_ends(piece: &Value) -> [Point; 4] {
    let geometry = &piece["three_way_geometry"];
    let length = geometry["length"].as_f64().unwrap_or_default();
    let branch_end = |branch: &Value, side: f64| -> Point {
        let radius = branch["radius"].as_f64().unwrap_or_default();
        let depart = branch["depart"].as_f64().unwrap_or_default();
        let sweep = match branch.get("sweep").and_then(Value::as_f64) {
            Some(degrees) => degrees.to_radians(),
            None => ((length - depart) / radius).asin(),
        };
        (depart + radius * sweep.sin(), side * radius * (1.0 - sweep.cos()))
    };
    [
        (0.0, 0.0),
        (length, 0.0),
        branch_end(&geometry["right"], 1.0),
        branch_end(&geometry["left"], -1.0),
    ]
}

/// The placement that puts four local ends on four world points
pub struct FourFit {
    pub origin: Point,
    pub rotation: f64,
    pub flip: bool,
    /// For world point k, the index of the local end that lands on it
    pub local_of_world: [usize; 4],
    pub error: f64,
}

/// All orders of 0 to 3, in lexicographic order
fn permutations() -> Vec<[usize; 4]> {
    let mut all = Vec::with_capacity(24);
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    if a != b && a != c && a != d && b != c && b != d && c != d {
                        all.push([a, b, c, d]);
                    }
                }
            }
        }
    }
    all
}

/// Try every pairing of the four ends and both hands, and keep the one with the least error
pub fn fit_four(local: &[Point], world: &[Point; 4]) -> FourFit {
    let centroid = |points: &[Point]| -> Point {
        (points.iter().map(|p| p.0).sum::<f64>() / 4.0, points.iter().map(|p| p.1).sum::<f64>() / 4.0)
    };
    let cw = centroid(world);
    let mut best: Option<FourFit> = None;
    for flip in [false, true] {
        let local_points: Vec<Point> = local.iter().map(|point| mirror(*point, flip)).collect();
        for order in permutations() {
            let paired: Vec<Point> = order.iter().map(|&i| local_points[i]).collect();
            let cl = centroid(&paired);
            let mut num = 0.0;
            let mut den = 0.0;
            for (q, w) in paired.iter().zip(world.iter()) {
                num += (q.0 - cl.0) * (w.1 - cw.1) - (q.1 - cl.1) * (w.0 - cw.0);
                den += (q.0 - cl.0) * (w.0 - cw.0) + (q.1 - cl.1) * (w.1 - cw.1);
            }
            let rotation = num.atan2(den).to_degrees();
            let (rx, ry) = rotate(cl, rotation);
            let origin = (cw.0 - rx, cw.1 - ry);
            let squared: f64 = paired
                .iter()
                .zip(world.iter())
                .map(|(q, w)| {
                    let placed = world_point(*q, origin, rotation);
                    (placed.0 - w.0).powi(2) + (placed.1 - w.1).powi(2)
                })
                .sum();
            let error = (squared / 4.0).sqrt();
            if best.as_ref().is_none_or(|fit| error < fit.error) {
                // order[k] is the local end paired with world point k
                best = Some(FourFit { origin, rotation, flip, local_of_world: order, error });
            }
        }
    }
    best.unwrap_or(FourFit { origin: (0.0, 0.0), rotation: 0.0, flip: false, local_of_world: [0, 1, 2, 3], error: f64::INFINITY })
}

fn world_point(point: Point, origin: Point, rotation: f64) -> Point {
    world(point, origin, rotation, false)
}

fn dist(a: Point, b: Point) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Curved flex track: a cubic Bezier from the origin to (length, 0), with two handles off the track. Matches the
/// layout editor's curves.js
pub const BEZIER_STEPS: usize = 24;

pub fn bezier_path(length: f64, handles: &[Point; 2]) -> Vec<Point> {
    let p0 = (0.0, 0.0);
    let (p1, p2, p3) = (handles[0], handles[1], (length, 0.0));
    (0..=BEZIER_STEPS)
        .map(|step| {
            let t = step as f64 / BEZIER_STEPS as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            (
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            )
        })
        .collect()
}

/// Handles for flex track whose path follows `samples` as closely as possible. The track leaves its start
/// at start_angle and arrives at its end at end_angle (radians). Each handle stays on its end's line
pub fn fit_handles(samples: &[Point], length: f64, start_angle: f64, end_angle: f64) -> [Point; 2] {
    let build = |r1: f64, r2: f64| -> [Point; 2] {
        [
            (r1 * start_angle.cos(), r1 * start_angle.sin()),
            (length - r2 * end_angle.cos(), -r2 * end_angle.sin()),
        ]
    };
    let cost = |r1: f64, r2: f64| -> f64 {
        let path = bezier_path(length, &build(r1, r2));
        samples
            .iter()
            .map(|sample| {
                let nearest = path.iter().map(|point| dist(*sample, *point)).fold(f64::INFINITY, f64::min);
                nearest * nearest
            })
            .sum()
    };

    let low = 0.02 * length;
    let mut best = (length / 3.0, length / 3.0, f64::INFINITY);
    let grid = 16;
    for i in 1..=grid {
        for j in 1..=grid {
            let r1 = low + (i as f64 / grid as f64) * (1.2 * length - low);
            let r2 = low + (j as f64 / grid as f64) * (1.2 * length - low);
            let value = cost(r1, r2);
            if value < best.2 {
                best = (r1, r2, value);
            }
        }
    }
    let mut spread = 1.2 * length / grid as f64;
    for _ in 0..4 {
        let (centre_1, centre_2) = (best.0, best.1);
        for i in -3..=3 {
            for j in -3..=3 {
                let r1 = (centre_1 + i as f64 * spread / 3.0).max(low);
                let r2 = (centre_2 + j as f64 * spread / 3.0).max(low);
                let value = cost(r1, r2);
                if value < best.2 {
                    best = (r1, r2, value);
                }
            }
        }
        spread /= 3.0;
    }
    build(best.0, best.1)
}

fn rotate_radians(point: Point, radians: f64) -> Point {
    let (sin, cos) = radians.sin_cos();
    (point.0 * cos - point.1 * sin, point.0 * sin + point.1 * cos)
}

/// The samples of a circular arc in the frame where its chord lies along x, with the chord and its angle, and the
/// start and end tangents relative to the chord. Returns (samples, chord, chord angle, start tangent, end tangent)
pub fn arc_bend_samples(radius: f64, sweep: f64, flip: bool, count: usize) -> (Vec<Point>, f64, f64, f64, f64) {
    let sign = if flip { -1.0 } else { 1.0 };
    let local: Vec<Point> = (0..count)
        .map(|k| {
            let phi = sweep * k as f64 / (count - 1) as f64;
            (radius * phi.sin(), sign * radius * (1.0 - phi.cos()))
        })
        .collect();
    let end = local[count - 1];
    let chord = end.0.hypot(end.1);
    let angle = end.1.atan2(end.0);
    let samples = local.iter().map(|point| rotate_radians(*point, -angle)).collect();
    (samples, chord, angle, -angle, sign * sweep - angle)
}

/// Samples of a cubic Bezier in the chord frame, with the chord, its angle, and the end tangents
pub fn bezier_bend_samples(start: Point, control_start: Point, control_end: Point, end: Point, count: usize) -> (Vec<Point>, f64, f64, f64, f64) {
    let point = |t: f64| -> Point {
        let u = 1.0 - t;
        (
            u.powi(3) * start.0 + 3.0 * u * u * t * control_start.0 + 3.0 * u * t * t * control_end.0 + t.powi(3) * end.0,
            u.powi(3) * start.1 + 3.0 * u * u * t * control_start.1 + 3.0 * u * t * t * control_end.1 + t.powi(3) * end.1,
        )
    };
    let chord_vector = (end.0 - start.0, end.1 - start.1);
    let chord = chord_vector.0.hypot(chord_vector.1);
    let angle = chord_vector.1.atan2(chord_vector.0);
    let samples = (0..count)
        .map(|k| {
            let p = point(k as f64 / (count - 1) as f64);
            rotate_radians((p.0 - start.0, p.1 - start.1), -angle)
        })
        .collect();
    let first = if dist(start, control_start) > 1e-6 { control_start } else { control_end };
    let last = if dist(end, control_end) > 1e-6 { control_end } else { control_start };
    let start_tangent = (first.1 - start.1).atan2(first.0 - start.0) - angle;
    let end_tangent = (end.1 - last.1).atan2(end.0 - last.0) - angle;
    (samples, chord, angle, start_tangent, end_tangent)
}
