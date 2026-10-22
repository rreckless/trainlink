//! Straight and curved track between the placed pieces, and the three-way and slip placements

use std::collections::{HashMap, HashSet};

use roxmltree::Document;
use serde_json::{json, Value};

use super::geometry::{angle, arc_bend_samples, bezier_bend_samples, fit_four, fit_handles, line_difference, three_way_ends, world, Point};
use super::{attr, record, round2, round4, segment_keys, Connectors, Key, GAP_REPORT_MM, SLIP_KEYS, SLIP_PIECE, STRAIGHT_PIECE, THREE_WAY_PIECE};
use crate::library::library;

pub const MIN_CURVE_MM: f64 = 1.0;


/// Directions of the straight segments at each anchor
pub fn segment_lines(doc: &Document, raw: &HashMap<Key, Point>) -> HashMap<Key, Vec<f64>> {
    let mut lines: HashMap<Key, Vec<f64>> = HashMap::new();
    for segment in doc.descendants().filter(|node| node.has_tag_name("tracksegment")) {
        if attr(&segment, "arc") == "yes" || attr(&segment, "bezier") == "yes" {
            continue;
        }
        let (start, end) = segment_keys(&segment);
        if let (Some(&from), Some(&to)) = (raw.get(&start), raw.get(&end)) {
            lines.entry(start).or_default().push(angle(from, to));
            lines.entry(end).or_default().push(angle(to, from));
        }
    }
    lines
}

/// Directions of the turnout legs at each anchor, for matching curve bulges
pub fn joint_lines(links: &HashMap<Key, Key>, heading_out: &HashMap<Key, f64>) -> HashMap<Key, Vec<f64>> {
    let mut lines: HashMap<Key, Vec<f64>> = HashMap::new();
    for (leg, heading) in heading_out {
        if let Some(link) = links.get(leg).filter(|link| link.1 == "POS_POINT") {
            lines.entry(link.clone()).or_default().push(*heading);
        }
    }
    lines
}

/// Which way a 90 degree arc bulges. Each option gives the heading it starts on, and its ends should carry on
/// the tangent of the track they join. Returns (flip, ambiguous)
pub fn choose_curve_flip(start: Point, end: Point, start_key: &Key, end_key: &Key, lines: &HashMap<Key, Vec<f64>>) -> (bool, bool) {
    let chord = angle(start, end);
    let mut scores = [(0.0f64, 0usize); 2];
    for flip in [false, true] {
        let heading = if flip { chord + 45.0 } else { chord - 45.0 };
        let end_heading = if flip { heading - 90.0 } else { heading + 90.0 };
        let (mut score, mut joined) = (0.0, 0);
        for (key, tangent) in [(start_key, heading), (end_key, end_heading)] {
            if let Some(candidates) = lines.get(key).filter(|candidates| !candidates.is_empty()) {
                score += candidates
                    .iter()
                    .map(|line| line_difference(tangent, *line))
                    .fold(f64::INFINITY, f64::min);
                joined += 1;
            }
        }
        scores[flip as usize] = (score, joined);
    }
    let [(false_score, false_joined), (true_score, true_joined)] = scores;
    if false_joined == 0 && true_joined == 0 {
        return (false, true);
    }
    (true_score < false_score, (false_score - true_score).abs() < 1.0)
}

/// Each placed three-way's outer legs: the record, the gaps, the turnouts it replaces, and each leg's exact point
pub struct ThreeWays {
    pub records: Vec<Value>,
    pub gaps: Vec<Value>,
    pub replaced: HashSet<String>,
    pub leg_points: HashMap<Key, Point>,
}

pub fn place_three_ways(groups: &[(Vec<String>, Vec<Key>)], raw: &HashMap<Key, Point>, scale: f64, links: &HashMap<Key, Key>) -> ThreeWays {
    let local = three_way_ends(&library().get(THREE_WAY_PIECE).cloned().unwrap_or(Value::Null));
    let local: Vec<Point> = local.to_vec();
    let mut result = ThreeWays { records: Vec::new(), gaps: Vec::new(), replaced: HashSet::new(), leg_points: HashMap::new() };
    for (idents, outer) in groups {
        let world_points: Vec<Point> = outer.iter().map(|leg| {
            let point = raw[&links[leg]];
            (point.0 * scale, point.1 * scale)
        }).collect();
        let world_points: [Point; 4] = [world_points[0], world_points[1], world_points[2], world_points[3]];
        let fit = fit_four(&local, &world_points);
        let ident = format!("{}-3way", idents[0]);
        result.records.push(record(&ident, THREE_WAY_PIECE, fit.origin, fit.rotation, fit.flip, Vec::new()));
        result.replaced.extend(idents.iter().cloned());
        for (index, leg) in outer.iter().enumerate() {
            let end = world(local[fit.local_of_world[index]], fit.origin, fit.rotation, fit.flip);
            result.leg_points.insert(links[leg].clone(), end);
            let gap = (end.0 - world_points[index].0).hypot(end.1 - world_points[index].1);
            if gap > GAP_REPORT_MM {
                result.gaps.push(json!({ "ident": ident, "joint": format!("{} {}", leg.0, leg.1), "gap_mm": round2(gap) }));
            }
        }
    }
    result
}

/// Slips placed from their four anchors. Returns the records, the gaps, the unplaced slips, the placed idents,
/// and each slip end's exact point keyed by the anchor its track joins
pub struct Slips {
    pub records: Vec<Value>,
    pub gaps: Vec<Value>,
    pub unplaced: Vec<Value>,
    pub placed: HashSet<String>,
    pub leg_points: HashMap<Key, Point>,
}

pub fn place_slips(doc: &Document, raw: &HashMap<Key, Point>, scale: f64, links: &HashMap<Key, Key>) -> Result<Slips, String> {
    let geometry = &library().get(SLIP_PIECE).cloned().unwrap_or(Value::Null)["slip_geometry"];
    let names = ["track_a_top", "track_b_top", "track_a_bottom", "track_b_bottom"];
    let local: Vec<Point> = names
        .iter()
        .map(|name| (geometry[*name][0].as_f64().unwrap_or_default(), geometry[*name][1].as_f64().unwrap_or_default()))
        .collect();
    let mut result = Slips { records: Vec::new(), gaps: Vec::new(), unplaced: Vec::new(), placed: HashSet::new(), leg_points: HashMap::new() };
    for slip in doc.descendants().filter(|node| node.has_tag_name("layoutSlip")) {
        let ident = attr(&slip, "ident").to_string();
        let keys: Vec<Key> = SLIP_KEYS.iter().map(|kind| (ident.clone(), kind.to_string())).collect();
        let anchors: Vec<Option<&Key>> = keys.iter().map(|leg| links.get(leg)).collect();
        let joined = anchors.iter().all(|link| link.is_some_and(|link| link.1 == "POS_POINT" && raw.contains_key(link)));
        if !joined {
            result.unplaced.push(json!({ "ident": ident, "kind": "double_slip", "reason": "a slip end is not joined to an anchor" }));
            continue;
        }
        let anchors: Vec<&Key> = anchors.into_iter().flatten().collect();
        let world_points: Vec<Point> = anchors.iter().map(|link| {
            let point = raw[*link];
            (point.0 * scale, point.1 * scale)
        }).collect();
        let world_points: [Point; 4] = [world_points[0], world_points[1], world_points[2], world_points[3]];
        let fit = fit_four(&local, &world_points);
        result.records.push(record(&ident, SLIP_PIECE, fit.origin, fit.rotation, fit.flip, Vec::new()));
        result.placed.insert(ident.clone());
        for (index, kind) in SLIP_KEYS.iter().enumerate() {
            let end = world(local[fit.local_of_world[index]], fit.origin, fit.rotation, fit.flip);
            result.leg_points.insert(anchors[index].clone(), end);
            let gap = (end.0 - world_points[index].0).hypot(end.1 - world_points[index].1);
            if gap > GAP_REPORT_MM {
                result.gaps.push(json!({ "ident": ident, "joint": format!("{ident} {kind}"), "gap_mm": round2(gap) }));
            }
        }
    }
    Ok(result)
}

/// Records for straight and curved segments, the ambiguous curves, and the unplaced segments
pub struct Segments {
    pub records: Vec<Value>,
    pub ambiguous: Vec<String>,
    pub unplaced: Vec<Value>,
}

/// A curve as flex track of its chord length, curved by two handles that follow it
fn curve_as_flex(ident: &str, origin: Point, rotation: f64, chord: f64, samples: &[Point], start: f64, end: f64) -> Value {
    let handles = fit_handles(samples, chord, start, end);
    let handle_values: Vec<Value> = handles.iter().map(|(x, y)| json!({ "x": round4(*x), "y": round4(*y) })).collect();
    record(ident, STRAIGHT_PIECE, origin, rotation, false, vec![("length", json!(round4(chord))), ("handles", Value::Array(handle_values))])
}

pub struct SegmentInputs<'a> {
    pub doc: &'a Document<'a>,
    pub raw: &'a HashMap<Key, Point>,
    pub radii_px: &'a HashMap<String, f64>,
    pub scale: f64,
    pub connectors: &'a Connectors,
    pub supported: &'a HashSet<String>,
    pub lines: &'a HashMap<Key, Vec<f64>>,
    pub joined_points: &'a HashMap<Key, Point>,
}

pub fn place_segments(input: &SegmentInputs) -> Result<Segments, String> {
    let mut result = Segments { records: Vec::new(), ambiguous: Vec::new(), unplaced: Vec::new() };
    for segment in input.doc.descendants().filter(|node| node.has_tag_name("tracksegment")) {
        let ident = attr(&segment, "ident").to_string();
        let arc = attr(&segment, "arc") == "yes";
        let bezier = attr(&segment, "bezier") == "yes";
        let kind = if arc || bezier { "curve" } else { "straight" };
        let (from, to) = segment_keys(&segment);
        let unplaced = |reason: &str| json!({ "ident": ident, "kind": kind, "reason": reason });

        if input.connectors.idents.contains(&ident) {
            // Connectors to turnouts and slips have no real length. Report only the ones that lead to an unplaced piece
            let leads_to_unplaced = [&from, &to]
                .iter()
                .any(|leg| (leg.1.starts_with("TURNOUT_") || leg.1.starts_with("SLIP_")) && !input.supported.contains(&leg.0));
            if leads_to_unplaced {
                result.unplaced.push(unplaced("connected to an unplaced turnout or slip"));
            }
            continue;
        }
        let (Some(&start_jmri), Some(&end_jmri)) = (input.raw.get(&from), input.raw.get(&to)) else {
            result.unplaced.push(unplaced("unknown end point"));
            continue;
        };
        let scaled = |point: Point| (point.0 * input.scale, point.1 * input.scale);
        let start = input.joined_points.get(&from).copied().unwrap_or_else(|| scaled(start_jmri));
        let end = input.joined_points.get(&to).copied().unwrap_or_else(|| scaled(end_jmri));

        if bezier {
            let mut controls: Vec<(i64, Point)> = segment
                .descendants()
                .filter(|node| node.has_tag_name("controlpoint"))
                .map(|node| Ok((node.attribute("index").unwrap_or_default().parse::<i64>().unwrap_or_default(), scaled(super::xy(&node, "x", "y")?))))
                .collect::<Result<_, String>>()?;
            controls.sort_by_key(|(index, _)| *index);
            if controls.len() != 2 {
                result.unplaced.push(unplaced("bezier without two control points"));
                continue;
            }
            let (samples, chord, chord_angle, start_angle, end_angle) = bezier_bend_samples(start, controls[0].1, controls[1].1, end, 24);
            if chord < MIN_CURVE_MM {
                result.unplaced.push(unplaced("bezier shorter than 1 mm"));
                continue;
            }
            result.records.push(curve_as_flex(&ident, start, chord_angle.to_degrees(), chord, &samples, start_angle, end_angle));
        } else if arc {
            let sweep_degrees = segment.attribute("angle").and_then(|text| text.parse::<f64>().ok()).unwrap_or_default();
            if sweep_degrees != 90.0 {
                result.unplaced.push(unplaced("only 90 degree curves are supported"));
                continue;
            }
            let radius_px = input.radii_px.get(&ident).copied().ok_or("A curve has no radius")?;
            let (flip, ambiguous) = choose_curve_flip(start_jmri, end_jmri, &from, &to, input.lines);
            let (samples, arc_chord, _, start_angle, end_angle) = arc_bend_samples(radius_px * input.scale, 90f64.to_radians(), flip, 24);
            // The ends may sit on turnout legs, a fraction of a mm from the arc's own chord. Fit the shape to them
            let chord = (end.0 - start.0).hypot(end.1 - start.1);
            let factor = chord / arc_chord;
            let samples: Vec<Point> = samples.iter().map(|(x, y)| (x * factor, y * factor)).collect();
            result.records.push(curve_as_flex(&ident, start, angle(start, end), chord, &samples, start_angle, end_angle));
            if ambiguous {
                result.ambiguous.push(ident.clone());
            }
        } else {
            let length = (end.0 - start.0).hypot(end.1 - start.1);
            result.records.push(record(&ident, STRAIGHT_PIECE, start, angle(start, end), false, vec![("length", json!(round4(length)))]));
        }
    }
    Ok(result)
}
