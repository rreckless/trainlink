//! AnyRail (JMRI layout-config XML) import, ported from the web backend's importer.
//!
//! JMRI coordinates are pixels with no real size, so the caller gives the real radius in mm of the innermost
//! curve. The smallest 90 degree arc in the file is taken to be that curve, and every coordinate is scaled by the
//! same factor. Anchor points are drawn to scale, but a turnout's own symbol points are not, so each turnout sits on
//! its anchors. Connector segments between a turnout and its anchors have no real length, so they are not placed.

pub mod geometry;
pub(crate) mod segments;

use segments::{joint_lines, place_segments, place_slips, place_three_ways, segment_lines, SegmentInputs};

use std::collections::{HashMap, HashSet};

use roxmltree::{Document, Node};
use serde_json::{json, Value};

use crate::library::library;
use geometry::{align, angle, normalize, turnout_ends, world, Point};

pub use segments::MIN_CURVE_MM;

pub type Key = (String, String);

pub const TURNOUT_KEYS: [&str; 3] = ["TURNOUT_A", "TURNOUT_B", "TURNOUT_C"];
pub const SLIP_KEYS: [&str; 4] = ["SLIP_A", "SLIP_B", "SLIP_C", "SLIP_D"];
pub const SLIP_PIECE: &str = "peco-n-sl-e390f";
pub const THREE_WAY_PIECE: &str = "peco-n-sl-e399f";
pub const STRAIGHT_PIECE: &str = "peco-n-sl-300f";
pub const GAP_REPORT_MM: f64 = 1.0;
pub const MARGIN_MM: f64 = 50.0;

fn turnout_piece(hand: &str) -> Option<&'static str> {
    match hand {
        "RH_TURNOUT" => Some("peco-n-sl-u395f"),
        "LH_TURNOUT" => Some("peco-n-sl-u396f"),
        _ => None,
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn key(ident: &str, kind: &str) -> Key {
    (ident.to_string(), kind.to_string())
}

/// The value a file refers to, or an error naming it. A file that refers to something it never defines is refused
/// with a message, instead of stopping the import
pub fn need<'a, V>(map: &'a HashMap<Key, V>, key: &Key) -> Result<&'a V, String> {
    map.get(key).ok_or_else(|| format!("The AnyRail file does not define {} {}", key.0, key.1))
}

fn attr<'a>(node: &Node<'a, '_>, name: &str) -> &'a str {
    node.attribute(name).unwrap_or_default()
}

fn number(node: &Node, name: &str) -> Result<f64, String> {
    node.attribute(name)
        .and_then(|text| text.trim().parse::<f64>().ok())
        .ok_or_else(|| format!("{} needs a number for {name}", node.tag_name().name()))
}

fn xy(node: &Node, x: &str, y: &str) -> Result<Point, String> {
    Ok((number(node, x)?, number(node, y)?))
}

/// The layout as the importer needs it: the parsed document plus the pixel position of every point
pub struct Layout<'a> {
    pub doc: Document<'a>,
    pub raw: HashMap<Key, Point>,
}

/// Parse the XML, refusing DOCTYPEs and ENTITY declarations (JMRI never writes them)
pub fn parse(text: &str) -> Result<Document<'_>, String> {
    if text.trim().is_empty() {
        return Err("No layout XML supplied".into());
    }
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("XML with a DOCTYPE or ENTITY declaration is not accepted".into());
    }
    let doc = Document::parse(text).map_err(|error| format!("Not valid XML: {error}"))?;
    let has_track = doc
        .descendants()
        .any(|node| node.has_tag_name("layoutturnout") || node.has_tag_name("tracksegment"));
    if !has_track {
        return Err("No layout track found in the XML".into());
    }
    Ok(doc)
}

pub fn raw_points(doc: &Document) -> Result<HashMap<Key, Point>, String> {
    let mut raw = HashMap::new();
    for point in doc.descendants().filter(|node| node.has_tag_name("positionablepoint")) {
        raw.insert(key(attr(&point, "ident"), "POS_POINT"), xy(&point, "x", "y")?);
    }
    for turnout in doc.descendants().filter(|node| node.has_tag_name("layoutturnout")) {
        let ident = attr(&turnout, "ident");
        let pairs = [("TURNOUT_A", "xcen", "ycen"), ("TURNOUT_B", "xb", "yb"), ("TURNOUT_C", "xc", "yc")];
        for (kind, x, y) in pairs {
            raw.insert(key(ident, kind), xy(&turnout, x, y)?);
        }
    }
    Ok(raw)
}

/// (from, to) keys of a segment. A missing name gives an empty key, which is never in the raw points
pub fn segment_keys(segment: &Node) -> (Key, Key) {
    (
        (attr(segment, "connect1name").to_string(), attr(segment, "type1").to_string()),
        (attr(segment, "connect2name").to_string(), attr(segment, "type2").to_string()),
    )
}

/// Pixel radius of each 90 degree arc, from its chord
pub fn arc_radii_px(doc: &Document, raw: &HashMap<Key, Point>) -> Result<HashMap<String, f64>, String> {
    let mut radii = HashMap::new();
    for segment in doc.descendants().filter(|node| node.has_tag_name("tracksegment")) {
        if attr(&segment, "arc") != "yes" {
            continue;
        }
        let (from, to) = segment_keys(&segment);
        let (Some(start), Some(end)) = (raw.get(&from), raw.get(&to)) else { continue };
        let half_angle = number(&segment, "angle")?.to_radians() / 2.0;
        radii.insert(attr(&segment, "ident").to_string(), (start.0 - end.0).hypot(start.1 - end.1) / (2.0 * half_angle.sin()));
    }
    Ok(radii)
}

/// Legs of turnouts and slips, each mapped to the other end of its connector segment, in file order
pub struct Connectors {
    pub links: HashMap<Key, Key>,
    pub order: Vec<Key>,
    pub idents: HashSet<String>,
}

pub fn connectors(doc: &Document) -> Connectors {
    let mut links = HashMap::new();
    let mut order = Vec::new();
    let mut idents = HashSet::new();
    let is_leg = |kind: &str| kind.starts_with("TURNOUT_") || kind.starts_with("SLIP_");
    for segment in doc.descendants().filter(|node| node.has_tag_name("tracksegment")) {
        let (first, second) = segment_keys(&segment);
        if !is_leg(&first.1) && !is_leg(&second.1) {
            continue;
        }
        idents.insert(attr(&segment, "ident").to_string());
        for (leg, other) in [(first.clone(), second.clone()), (second, first)] {
            if is_leg(&leg.1) {
                if links.insert(leg.clone(), other).is_none() {
                    order.push(leg);
                }
            }
        }
    }
    Connectors { links, order, idents }
}

/// A piece record in the shape the editor stores: placed at origin, turned by rotation, flipped or not
pub fn record(ident: &str, piece_id: &str, origin: Point, rotation: f64, flip: bool, dimensions: Vec<(&str, Value)>) -> Value {
    let mut value = json!({
        "id": format!("jmri-{ident}"),
        "piece_id": piece_id,
        "x": round4(origin.0),
        "y": round4(origin.1),
        "rotation": round4(normalize(rotation)),
        "flip": flip,
    });
    for (name, field) in dimensions {
        value[name] = field;
    }
    value
}

/// A turnout placed from its anchors, with the outward heading of each leg
pub struct Turnouts {
    pub records: Vec<Value>,
    pub ends: HashMap<Key, Point>,
    pub heading_out: HashMap<Key, f64>,
    pub mismatches: Vec<String>,
    pub unplaced: Vec<Value>,
    pub unanchored: Vec<String>,
    pub gaps: Vec<Value>,
    pub placed: HashSet<String>,
}

pub fn place_turnouts(doc: &Document, raw: &HashMap<Key, Point>, scale: f64, links: &HashMap<Key, Key>, skip: &HashSet<String>) -> Result<Turnouts, String> {
    struct Spec {
        ident: String,
        piece_id: &'static str,
        keys: [Key; 3],
        symbol: (Point, Point),
    }
    let mut specs: Vec<Spec> = Vec::new();
    let mut unplaced = Vec::new();
    let mut mismatches = Vec::new();
    for turnout in doc.descendants().filter(|node| node.has_tag_name("layoutturnout")) {
        let ident = attr(&turnout, "ident").to_string();
        let hand = attr(&turnout, "type");
        if skip.contains(&ident) {
            continue;
        }
        let Some(piece_id) = turnout_piece(hand) else {
            unplaced.push(json!({ "ident": ident, "kind": "turnout", "reason": "unknown turnout type" }));
            continue;
        };
        let (a, b, c) = (
            *need(raw, &key(&ident, "TURNOUT_A"))?,
            *need(raw, &key(&ident, "TURNOUT_B"))?,
            *need(raw, &key(&ident, "TURNOUT_C"))?,
        );
        // Positive when the diverging track turns toward +y, the side a right-hand piece is drawn on
        let cross = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if (cross > 0.0) != (hand == "RH_TURNOUT") {
            mismatches.push(ident.clone());
        }
        let keys = [key(&ident, TURNOUT_KEYS[0]), key(&ident, TURNOUT_KEYS[1]), key(&ident, TURNOUT_KEYS[2])];
        specs.push(Spec { ident, piece_id, keys, symbol: (a, b) });
    }

    let mut records = Vec::new();
    let mut ends: HashMap<Key, Point> = HashMap::new();
    let mut heading_out: HashMap<Key, f64> = HashMap::new();
    let mut pending: Vec<usize> = (0..specs.len()).collect();
    let library = library();
    let anchor = |leg: &Key| -> Option<Point> {
        let link = links.get(leg)?;
        if link.1 != "POS_POINT" {
            return None;
        }
        raw.get(link).map(|point| (point.0 * scale, point.1 * scale))
    };

    let mut progress = true;
    while !pending.is_empty() && progress {
        progress = false;
        for index in pending.clone() {
            let spec = &specs[index];
            let piece = library.get(spec.piece_id).cloned().unwrap_or(Value::Null);
            let piece_ends = turnout_ends(&piece);
            let anchored: Vec<(usize, Point)> = spec
                .keys
                .iter()
                .enumerate()
                .filter_map(|(position, leg)| anchor(leg).map(|point| (position, point)))
                .collect();
            if anchored.len() >= 2 {
                let (i, world_i) = anchored[0];
                let (j, world_j) = anchored[1];
                let (local_i, local_j) = (piece_ends[i].0, piece_ends[j].0);
                let rotation = angle(world_i, world_j) - angle(local_i, local_j);
                let (rx, ry) = geometry::rotate(local_i, rotation);
                let origin = (world_i.0 - rx, world_i.1 - ry);
                place(&mut records, &mut ends, &mut heading_out, spec.ident.clone(), spec.piece_id, &spec.keys, &piece_ends, origin, rotation);
                pending.retain(|&p| p != index);
                progress = true;
                continue;
            }
            // Otherwise join a leg to a turnout that is already placed
            for (position, leg) in spec.keys.iter().enumerate() {
                let Some(other) = links.get(leg) else { continue };
                if other.1.starts_with("TURNOUT_") && ends.contains_key(other) {
                    let heading = heading_out[other] + 180.0;
                    let (origin, rotation) = align(&piece_ends, position, ends[other], heading);
                    place(&mut records, &mut ends, &mut heading_out, spec.ident.clone(), spec.piece_id, &spec.keys, &piece_ends, origin, rotation);
                    pending.retain(|&p| p != index);
                    progress = true;
                    break;
                }
            }
        }
    }

    // Turnouts that touch no anchor and no placed turnout: placed from the file's own symbol
    let mut unanchored = Vec::new();
    for index in pending.clone() {
        let spec = &specs[index];
        let piece = library.get(spec.piece_id).cloned().unwrap_or(Value::Null);
        let piece_ends = turnout_ends(&piece);
        let (a, b) = spec.symbol;
        unanchored.push(spec.ident.clone());
        let origin = (a.0 * scale, a.1 * scale);
        place(&mut records, &mut ends, &mut heading_out, spec.ident.clone(), spec.piece_id, &spec.keys, &piece_ends, origin, angle(a, b));
    }

    let mut gaps = Vec::new();
    for spec in &specs {
        for leg in &spec.keys {
            let Some(link) = links.get(leg) else { continue };
            let target = if link.1 == "POS_POINT" && raw.contains_key(link) {
                let point = raw[link];
                (point.0 * scale, point.1 * scale)
            } else if link.1.starts_with("TURNOUT_") && ends.contains_key(link) && leg < link {
                ends[link]
            } else {
                continue;
            };
            let gap = (ends[leg].0 - target.0).hypot(ends[leg].1 - target.1);
            if gap > GAP_REPORT_MM {
                gaps.push(json!({ "ident": spec.ident, "joint": format!("{} {}", spec.ident, leg.1), "gap_mm": round2(gap) }));
            }
        }
    }

    let placed = specs.iter().map(|spec| spec.ident.clone()).collect();
    Ok(Turnouts { records, ends, heading_out, mismatches, unplaced, unanchored, gaps, placed })
}

#[allow(clippy::too_many_arguments)]
fn place(
    records: &mut Vec<Value>,
    ends: &mut HashMap<Key, Point>,
    heading_out: &mut HashMap<Key, f64>,
    ident: String,
    piece_id: &str,
    keys: &[Key; 3],
    piece_ends: &[(Point, f64); 3],
    origin: Point,
    rotation: f64,
) {
    records.push(record(&ident, piece_id, origin, rotation, false, Vec::new()));
    for (index, leg) in keys.iter().enumerate() {
        let (local_point, local_heading) = piece_ends[index];
        ends.insert(leg.clone(), world(local_point, origin, rotation, false));
        heading_out.insert(leg.clone(), normalize(rotation + local_heading));
    }
}

/// Pairs of turnouts joined directly to each other, with their other four legs all going to anchors
pub fn three_way_groups(doc: &Document, connectors: &Connectors) -> Vec<(Vec<String>, Vec<Key>)> {
    let turnouts: HashSet<String> = doc
        .descendants()
        .filter(|node| node.has_tag_name("layoutturnout"))
        .map(|node| attr(&node, "ident").to_string())
        .collect();
    let mut groups = Vec::new();
    let mut used: HashSet<String> = HashSet::new();
    for leg in &connectors.order {
        let Some(other) = connectors.links.get(leg) else { continue };
        if !(leg.1.starts_with("TURNOUT_") && other.1.starts_with("TURNOUT_")) {
            continue;
        }
        let pair: HashSet<String> = [leg.0.clone(), other.0.clone()].into_iter().collect();
        if pair.len() != 2 || !pair.is_subset(&turnouts) || !pair.is_disjoint(&used) {
            continue;
        }
        let mut sorted: Vec<String> = pair.iter().cloned().collect();
        sorted.sort();
        let outer: Vec<Key> = sorted
            .iter()
            .flat_map(|ident| TURNOUT_KEYS.iter().map(move |kind| key(ident, kind)))
            .filter(|candidate| candidate != leg && candidate != other)
            .collect();
        let all_anchored = outer
            .iter()
            .all(|leg| connectors.links.get(leg).is_some_and(|link| link.1 == "POS_POINT"));
        if all_anchored {
            used.extend(pair);
            groups.push((sorted, outer));
        }
    }
    groups
}

/// Convert a JMRI layout-config XML string into track pieces, with the layout's size and what could not be placed
pub fn import(xml: &str, inner_radius_mm: f64) -> Result<Value, String> {
    if !(inner_radius_mm > 0.0) {
        return Err("The inner curve radius must be a positive number of mm".into());
    }
    let doc = parse(xml)?;
    let raw = raw_points(&doc)?;
    let radii_px = arc_radii_px(&doc, &raw)?;
    let smallest = radii_px.values().copied().fold(f64::INFINITY, f64::min);
    if !smallest.is_finite() {
        return Err("No 90 degree curves in the layout, so the scale cannot be worked out".into());
    }
    let scale = inner_radius_mm / smallest;

    let connectors = connectors(&doc);
    let groups = three_way_groups(&doc, &connectors);
    let group_members: HashSet<String> = groups.iter().flat_map(|(idents, _)| idents.iter().cloned()).collect();
    let turnouts = place_turnouts(&doc, &raw, scale, &connectors.links, &group_members)?;
    let three = place_three_ways(&groups, &raw, scale, &connectors.links)?;
    let slips = place_slips(&doc, &raw, scale, &connectors.links)?;

    let mut gaps = turnouts.gaps.clone();
    gaps.extend(slips.gaps.iter().cloned());
    gaps.extend(three.gaps.iter().cloned());
    let mut unplaced = turnouts.unplaced.clone();
    unplaced.extend(slips.unplaced.iter().cloned());
    let mut supported = turnouts.placed.clone();
    supported.extend(slips.placed.iter().cloned());
    supported.extend(three.replaced.iter().cloned());

    let mut lines = segment_lines(&doc, &raw);
    for (anchor, headings) in joint_lines(&connectors.links, &turnouts.heading_out) {
        lines.entry(anchor).or_default().extend(headings);
    }

    // Track ends that join a turnout, slip, or three-way leg take that leg's own point
    let mut joined_points: HashMap<Key, Point> = HashMap::new();
    for (leg, anchor) in &connectors.links {
        if let (Some(point), true) = (turnouts.ends.get(leg), anchor.1 == "POS_POINT") {
            joined_points.insert(anchor.clone(), *point);
        }
    }
    joined_points.extend(slips.leg_points.iter().map(|(anchor, point)| (anchor.clone(), *point)));
    joined_points.extend(three.leg_points.iter().map(|(anchor, point)| (anchor.clone(), *point)));

    let segments = place_segments(&SegmentInputs {
        doc: &doc,
        raw: &raw,
        radii_px: &radii_px,
        scale,
        connectors: &connectors,
        supported: &supported,
        lines: &lines,
        joined_points: &joined_points,
    })?;
    unplaced.extend(segments.unplaced.iter().cloned());

    let mut pieces: Vec<Value> = Vec::new();
    pieces.extend(turnouts.records);
    pieces.extend(three.records);
    pieces.extend(slips.records);
    pieces.extend(segments.records);

    // Move the layout so it starts at the margin, and size the layout to fit it
    let anchors: Vec<Point> = raw
        .iter()
        .filter(|(anchor, _)| anchor.1 == "POS_POINT")
        .map(|(_, point)| (point.0 * scale, point.1 * scale))
        .collect();
    let xs: Vec<f64> = anchors.iter().map(|p| p.0).chain(pieces.iter().map(|p| p["x"].as_f64().unwrap_or_default())).collect();
    let ys: Vec<f64> = anchors.iter().map(|p| p.1).chain(pieces.iter().map(|p| p["y"].as_f64().unwrap_or_default())).collect();
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

    gaps.sort_by(|a, b| {
        b["gap_mm"].as_f64().unwrap_or_default().total_cmp(&a["gap_mm"].as_f64().unwrap_or_default())
    });
    Ok(json!({
        "track_pieces": pieces,
        "width": (max_x - min_x + 2.0 * MARGIN_MM).ceil() as i64,
        "height": (max_y - min_y + 2.0 * MARGIN_MM).ceil() as i64,
        "track_system": "Code 55",
        "unplaced": unplaced,
        "scale_mm_per_pixel": round4(scale),
        "hand_mismatches": turnouts.mismatches,
        "unanchored_turnouts": turnouts.unanchored,
        "ambiguous_curves": segments.ambiguous,
        "gaps": gaps,
    }))
}
