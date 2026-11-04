//! The newer AnyRail export, OMLIF JSON. Each part lists its ends, with a coordinate and a heading, and its drawing.
//! The coordinates carry their units, so no scale is fitted, and each piece is placed from its own ends the way the XML
//! import places them.
//!
//! The schema's end headings do not follow one rule across the file, so the importer does not use them. A straight's
//! two ends give its line, and a turnout is placed by finding the entry, straight and diverging ends whose spacing and
//! angle match a Peco turnout of one hand. Curves are fitted from their arcs or beziers, as the XML import does.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{json, Value};

use super::geometry::{angle, arc_bend_samples, bezier_bend_samples, fit_four, normalize, rotate, three_way_ends, turnout_ends, world, Point};
use super::segments::curve_as_flex;
use super::{record, round2, round4, GAP_REPORT_MM, MARGIN_MM, SLIP_PIECE, STRAIGHT_PIECE, THREE_WAY_PIECE};
use crate::library::library;

/// How far a placed end may sit from the file's coordinate, in mm
const TOLERANCE_MM: f64 = 1.0;
/// The shortest piece that is placed, in mm
const MIN_LENGTH_MM: f64 = 1.0;

#[derive(Deserialize)]
struct File {
    layout: Layout,
}

#[derive(Deserialize)]
struct Layout {
    attributes: Attributes,
    endpoints: Vec<Endpoint>,
    parts: Vec<Part>,
    #[serde(default)]
    connections: Vec<[i64; 2]>,
}

#[derive(Deserialize)]
struct Attributes {
    units: String,
}

#[derive(Deserialize)]
struct Endpoint {
    coord: Vec<f64>,
    direction: f64,
    id: i64,
}

#[derive(Deserialize)]
struct Part {
    attributes: PartAttributes,
    drawing: Option<Drawing>,
    #[serde(rename = "endpoint-ids")]
    endpoint_ids: Vec<i64>,
}

#[derive(Deserialize)]
struct PartAttributes {
    hidden: bool,
    id: i64,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize, Default)]
struct Drawing {
    #[serde(default)]
    arcs: Vec<Arc>,
    #[serde(default)]
    beziers: Vec<Bezier>,
}

#[derive(Deserialize)]
struct Arc {
    angle: f64,
    center: Vec<f64>,
    p1: Vec<f64>,
    p2: Vec<f64>,
    radius: f64,
}

#[derive(Deserialize)]
struct Bezier {
    cp1: Vec<f64>,
    cp2: Vec<f64>,
    cp3: Vec<f64>,
    cp4: Vec<f64>,
}

/// An end in millimetres: where it is, and the heading of travel leaving the part through it, in degrees
type End = (Point, f64);

/// What the import needs while it places each part
struct Context {
    ends: HashMap<i64, End>,
    factor: f64,
    guessed_hand: Vec<String>,
    gaps: Vec<Value>,
    /// The exact point of each end of a turnout, slip or three-way, as the library geometry puts it
    exact: HashMap<i64, Point>,
}

/// Import an OMLIF JSON layout. Returns the same shape as the XML import, so the editor shows it the same way
pub fn import(text: &str) -> Result<Value, String> {
    // A byte-order mark is not JSON, so it is taken off first
    let text = text.trim_start_matches('\u{feff}');
    let file: File = serde_json::from_str(text).map_err(|error| {
        if error.is_eof() {
            "The file ends early, so it is incomplete. Copy it again, and make sure it has finished syncing".to_string()
        } else {
            format!("Not a valid AnyRail JSON layout: {error}")
        }
    })?;
    let layout = file.layout;
    let factor = units_to_mm(&layout.attributes.units)?;
    let mut ends = HashMap::new();
    for end in &layout.endpoints {
        let [x, y, ..] = end.coord[..] else {
            return Err(format!("Endpoint {} has no x and y", end.id));
        };
        ends.insert(end.id, ((x * factor, y * factor), normalize(end.direction)));
    }
    let mut context = Context { ends, factor, guessed_hand: Vec::new(), gaps: Vec::new(), exact: HashMap::new() };

    let points: Vec<Point> = context.ends.values().map(|(point, _)| *point).collect();
    let mut pieces: Vec<Value> = Vec::new();
    let mut unplaced: Vec<Value> = Vec::new();
    let mut skipped: Vec<Value> = Vec::new();
    // Turnouts, slips and three-ways go first. Their ends are where the library geometry puts them, which is a fraction
    // of a mm from the file's coordinates, so the track that joins them starts from those exact points, as the XML import
    // does. Everything else is placed in a second pass
    for joints_pass in [true, false] {
        for part in layout.parts.iter().filter(|part| is_joint(part.attributes.kind.as_str()) == joints_pass) {
            // Joints are fitted from the file's own coordinates. Track that joins them takes their exact points, below
            if !joints_pass {
                apply_joints(&mut context, &layout.connections);
            }
            let ident = format!("omlif-{}", part.attributes.id);
            let kind = part.attributes.kind.as_str();
            if part.attributes.hidden {
                skipped.push(json!({ "ident": ident, "kind": kind, "reason": "hidden" }));
                continue;
            }
            let placed = match kind {
                // Buffers and stubs are the ends of the track, not pieces of it
                "buffer" | "stubs" => {
                    skipped.push(json!({ "ident": ident, "kind": kind, "reason": "an end of the track, not a piece" }));
                    continue;
                }
                "straight" | "flex" if has_curve(part) => curve(&ident, part, &context),
                "straight" | "flex" => straight(&ident, part, &context),
                "curve" => curve(&ident, part, &context),
                "turnout" | "turnout-left" | "turnout-right" => turnout(&ident, part, &mut context),
                "slipswitch" => slip(&ident, part, &mut context),
                "threeway" => three_way(&ident, part, &mut context),
                _ => Err(format!("{kind} is not imported yet")),
            };
            match placed {
                Ok(piece) => pieces.push(piece),
                Err(reason) => unplaced.push(json!({ "ident": ident, "kind": kind, "reason": reason })),
            }
        }
    }

    // Move the layout so it starts at the margin, and size it to fit the track
    let mut xs: Vec<f64> = points.iter().map(|point| point.0).collect();
    let mut ys: Vec<f64> = points.iter().map(|point| point.1).collect();
    xs.extend(pieces.iter().map(|piece| piece["x"].as_f64().unwrap_or_default()));
    ys.extend(pieces.iter().map(|piece| piece["y"].as_f64().unwrap_or_default()));
    if xs.is_empty() {
        return Err("No track to import".into());
    }
    let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (dx, dy) = (MARGIN_MM - min_x, MARGIN_MM - min_y);
    for piece in pieces.iter_mut() {
        piece["x"] = json!(round4(piece["x"].as_f64().unwrap_or_default() + dx));
        piece["y"] = json!(round4(piece["y"].as_f64().unwrap_or_default() + dy));
    }
    Ok(json!({
        "track_pieces": pieces,
        "width": (max_x - min_x + 2.0 * MARGIN_MM).ceil() as i64,
        "height": (max_y - min_y + 2.0 * MARGIN_MM).ceil() as i64,
        "track_system": "Code 55",
        "unplaced": unplaced,
        "skipped": skipped,
        "guessed_hand": context.guessed_hand,
        "format": "omlif",
        "scale_mm_per_pixel": 1.0,
        "hand_mismatches": [],
        "unanchored_turnouts": [],
        "ambiguous_curves": [],
        "gaps": context.gaps,
    }))
}

/// An end joined to one that already has its exact point takes that point, so the two meet exactly
fn apply_joints(context: &mut Context, connections: &[[i64; 2]]) {
    for [a, b] in connections {
        for (from, to) in [(a, b), (b, a)] {
            if let Some(point) = context.exact.get(from).copied() {
                if !context.exact.contains_key(to) {
                    context.exact.insert(*to, point);
                    if let Some(end) = context.ends.get_mut(to) {
                        end.0 = point;
                    }
                }
            }
        }
    }
}

/// Turnouts, slips and three-ways: the parts whose ends are fixed by the library geometry
fn is_joint(kind: &str) -> bool {
    matches!(kind, "turnout" | "turnout-left" | "turnout-right" | "slipswitch" | "threeway")
}

fn units_to_mm(units: &str) -> Result<f64, String> {
    match units.trim().to_ascii_lowercase().as_str() {
        "mm" => Ok(1.0),
        "cm" => Ok(10.0),
        "m" => Ok(1000.0),
        "in" | "inch" | "inches" => Ok(25.4),
        other => Err(format!("Unknown units \"{other}\" in the file, so the sizes cannot be read")),
    }
}

fn dist(a: Point, b: Point) -> f64 {
    (b.0 - a.0).hypot(b.1 - a.1)
}

/// The ends of a part, in the order the file lists them. Each must be in the file and must have a coordinate
fn part_ends(part: &Part, context: &Context, count: usize) -> Result<Vec<End>, String> {
    if part.endpoint_ids.len() != count {
        return Err(format!("expected {count} ends, found {}", part.endpoint_ids.len()));
    }
    part.endpoint_ids
        .iter()
        .map(|id| context.ends.get(id).copied().ok_or_else(|| format!("end {id} is not in the file")))
        .collect()
}

/// Whether a flex part is drawn as a curve, with an arc or a bezier, rather than a straight
fn has_curve(part: &Part) -> bool {
    part.drawing.as_ref().is_some_and(|drawing| !drawing.arcs.is_empty() || !drawing.beziers.is_empty())
}

/// A straight, as flex track from its first end, turned to point at its second
fn straight(ident: &str, part: &Part, context: &Context) -> Result<Value, String> {
    let ends = part_ends(part, context, 2)?;
    let (start, end) = (ends[0].0, ends[1].0);
    let length = dist(start, end);
    if length < MIN_LENGTH_MM {
        return Err("the ends are at the same point".into());
    }
    Ok(record(ident, STRAIGHT_PIECE, start, angle(start, end), false, vec![("length", json!(round4(length)))]))
}

/// A curve, as flex track of its chord with handles that follow the arc or bezier it is drawn as
fn curve(ident: &str, part: &Part, context: &Context) -> Result<Value, String> {
    let ends = part_ends(part, context, 2)?;
    let (start, end) = (ends[0].0, ends[1].0);
    let chord = dist(start, end);
    if chord < MIN_LENGTH_MM {
        return Err("the curve's ends are at the same point".into());
    }
    let drawing = part.drawing.as_ref().ok_or("the curve has no drawing")?;
    let chord_angle = angle(start, end);
    if let Some(arc) = drawing.arcs.first() {
        let [cx, cy, ..] = arc.center[..] else {
            return Err("the arc has no centre".into());
        };
        if !(arc.angle > 0.0 && arc.angle <= 90.0) {
            return Err("only curves of up to 90 degrees are supported".into());
        }
        // The arc bends away from its centre: flip it when the centre lies on the other side of the chord
        let local_centre = rotate((cx * context.factor - start.0, cy * context.factor - start.1), -chord_angle);
        let flip = local_centre.1 < 0.0;
        let (samples, arc_chord, _, start_tangent, end_tangent) =
            arc_bend_samples(arc.radius * context.factor, arc.angle.to_radians(), flip, 24);
        let fit = chord / arc_chord;
        let samples: Vec<Point> = samples.iter().map(|(x, y)| (x * fit, y * fit)).collect();
        return Ok(curve_as_flex(ident, start, chord_angle, chord, &samples, start_tangent, end_tangent));
    }
    if let Some(bezier) = drawing.beziers.first() {
        let point = |coord: &[f64]| -> Result<Point, String> {
            let [x, y, ..] = coord[..] else {
                return Err("a control point has no x and y".into());
            };
            Ok((x * context.factor, y * context.factor))
        };
        let (first, second, third, last) = (point(&bezier.cp1)?, point(&bezier.cp2)?, point(&bezier.cp3)?, point(&bezier.cp4)?);
        // The control points may be listed from either end
        let (origin, far, near_control, far_control) = if dist(first, start) <= TOLERANCE_MM && dist(last, end) <= TOLERANCE_MM {
            (start, end, second, third)
        } else if dist(first, end) <= TOLERANCE_MM && dist(last, start) <= TOLERANCE_MM {
            (end, start, third, second)
        } else {
            return Err("the curve's control points do not start and end at its ends".into());
        };
        let (samples, bezier_chord, bezier_angle, start_tangent, end_tangent) = bezier_bend_samples(origin, near_control, far_control, far, 24);
        if bezier_chord < MIN_LENGTH_MM {
            return Err("the curve is shorter than 1 mm".into());
        }
        return Ok(curve_as_flex(ident, origin, bezier_angle.to_degrees(), bezier_chord, &samples, start_tangent, end_tangent));
    }
    Err("the curve has no arc or bezier".into())
}

/// A turnout, from its three ends. Each end could be the entry, with another as the straight exit and the third as the
/// diverging exit. A Peco turnout's straight is collinear with its entry, so the entry and straight exit fix its axis,
/// and the diverging end must then fit one hand's library geometry within TOLERANCE_MM.
///
/// Points alone do not settle the hand: a right-hand turnout's diverging leg is almost the length of its straight, so the
/// same three points can also fit a left-hand turnout. The file's hand is used when it gives one. Otherwise the drawing's
/// arc, which joins the entry to the diverging end, decides. When neither does, the turnout is refused rather than guessed
fn turnout(ident: &str, part: &Part, context: &mut Context) -> Result<Value, String> {
    let ends = part_ends(part, context, 3)?;
    let typed: Option<bool> = match part.attributes.kind.as_str() {
        "turnout-right" => Some(true),
        "turnout-left" => Some(false),
        _ => None,
    };
    let library = library();
    // Each fit: (right hand, entry index, diverging index, origin, rotation, piece id)
    let mut fits: Vec<(bool, usize, usize, Point, f64, &'static str)> = Vec::new();
    for entry in 0..3 {
        for straight in (0..3).filter(|i| *i != entry) {
            let diverge = 3 - entry - straight;
            let origin = ends[entry].0;
            let rotation = angle(origin, ends[straight].0);
            for (right, piece_id) in [(true, "peco-n-sl-u395f"), (false, "peco-n-sl-u396f")] {
                let Some(piece) = library.get(piece_id) else { continue };
                let local = turnout_ends(piece);
                if (dist(origin, ends[straight].0) - local[1].0 .0).abs() > TOLERANCE_MM {
                    continue;
                }
                let placed = world(local[2].0, origin, rotation, false);
                if dist(placed, ends[diverge].0) <= TOLERANCE_MM {
                    fits.push((right, entry, diverge, origin, rotation, piece_id));
                }
            }
        }
    }
    if fits.is_empty() {
        return Err("the turnout's legs do not fit a Peco Code 55 turnout".into());
    }

    // The drawing's arc joins the entry to the diverging end, when the file draws one
    let arc = part.drawing.as_ref().and_then(|drawing| drawing.arcs.first());
    if let Some(arc) = arc {
        let scaled = |coord: &[f64]| -> Result<Point, String> {
            let [x, y, ..] = coord[..] else {
                return Err("the turnout's arc has no x and y".into());
            };
            Ok((x * context.factor, y * context.factor))
        };
        let (first, second) = (scaled(&arc.p1)?, scaled(&arc.p2)?);
        // The arc starts at the entry and ends on the diverging side, which can stop a little short of the diverging end
        let joins = |entry: usize, straight: usize, diverge: usize| {
            let far = if dist(first, ends[entry].0) <= TOLERANCE_MM {
                second
            } else if dist(second, ends[entry].0) <= TOLERANCE_MM {
                first
            } else {
                return false;
            };
            dist(far, ends[diverge].0) < dist(far, ends[straight].0)
        };
        fits.retain(|fit| joins(fit.1, 3 - fit.1 - fit.2, fit.2));
        if fits.is_empty() {
            return Err("the turnout's arc does not join its entry to its diverging end".into());
        }
    }

    let hand_fits: Vec<_> = match typed {
        Some(hand) => fits.iter().copied().filter(|fit| fit.0 == hand).collect(),
        None => fits.clone(),
    };
    if hand_fits.is_empty() {
        return Err("the turnout's type does not match its geometry, so its hand is not known".into());
    }
    let mut distinct: Vec<&str> = hand_fits.iter().map(|fit| fit.5).collect();
    distinct.sort_unstable();
    distinct.dedup();
    if distinct.len() > 1 {
        return Err("the turnout could be either hand, and its drawing does not say which".into());
    }
    let (_, entry, diverge, origin, rotation, piece_id) = hand_fits[0];
    if typed.is_none() {
        context.guessed_hand.push(ident.to_string());
    }
    // The library's own ends, exactly where this turnout puts them, are what the track that joins it must meet
    let piece = library.get(piece_id).cloned().ok_or("the library has no turnout")?;
    let local = turnout_ends(&piece);
    let straight = 3 - entry - diverge;
    for (index, leg) in [(entry, 0), (straight, 1), (diverge, 2)] {
        context.exact.insert(part.endpoint_ids[index], world(local[leg].0, origin, rotation, false));
    }
    Ok(record(ident, piece_id, origin, rotation, false, Vec::new()))
}

/// A double slip, placed from its four ends as the XML import places one
fn slip(ident: &str, part: &Part, context: &mut Context) -> Result<Value, String> {
    let ends = part_ends(part, context, 4)?;
    let geometry = library().get(SLIP_PIECE).cloned().unwrap_or(Value::Null)["slip_geometry"].clone();
    let names = ["track_a_top", "track_b_top", "track_a_bottom", "track_b_bottom"];
    let local: Vec<Point> = names
        .iter()
        .map(|name| (geometry[*name][0].as_f64().unwrap_or_default(), geometry[*name][1].as_f64().unwrap_or_default()))
        .collect();
    if geometry.is_null() {
        return Err("the library has no slip geometry".into());
    }
    let joints = ["SLIP_A", "SLIP_B", "SLIP_C", "SLIP_D"].map(|kind| format!("{ident} {kind}"));
    Ok(four_point_piece(ident, &ends, &part.endpoint_ids, &local, SLIP_PIECE, &joints, context))
}

/// A three-way, from its four ends: the toe, the straight exit, and the two branches. The library's geometry is fitted
/// to them, so the three-way is calculated from the track that enters and leaves it
fn three_way(ident: &str, part: &Part, context: &mut Context) -> Result<Value, String> {
    let ends = part_ends(part, context, 4)?;
    let library_piece = library().get(THREE_WAY_PIECE).cloned().ok_or("the library has no three-way geometry")?;
    let local: Vec<Point> = three_way_ends(&library_piece).to_vec();
    let joints = ["toe", "straight", "right", "left"].map(|name| format!("{ident} {name}"));
    Ok(four_point_piece(ident, &ends, &part.endpoint_ids, &local, THREE_WAY_PIECE, &joints, context))
}

/// A piece with four ends in its own frame, placed where its four ends fit the file's four. Any end more than
/// GAP_REPORT_MM from its point is reported as a gap, as the XML import reports them
fn four_point_piece(ident: &str, ends: &[End], ids: &[i64], local: &[Point], piece_id: &str, joints: &[String; 4], context: &mut Context) -> Value {
    let world_points = [ends[0].0, ends[1].0, ends[2].0, ends[3].0];
    let fit = fit_four(local, &world_points);
    for (index, point) in world_points.iter().enumerate() {
        let placed = world(local[fit.local_of_world[index]], fit.origin, fit.rotation, fit.flip);
        context.exact.insert(ids[index], placed);
        let gap = dist(placed, *point);
        if gap > GAP_REPORT_MM {
            context.gaps.push(json!({ "ident": ident, "joint": joints[index], "gap_mm": round2(gap) }));
        }
    }
    record(ident, piece_id, fit.origin, fit.rotation, fit.flip, Vec::new())
}
