//! The AnyRail importer checked against the web backend's Python output for the same files

use serde_json::{json, Value};

use crate::anyrail::{import, legs_without_track};

const FIXTURES: [(&str, &str, &str); 5] = [
    ("basic", include_str!("anyrail/basic.xml"), include_str!("anyrail/basic.json")),
    ("turnout_joined", include_str!("anyrail/turnout_joined.xml"), include_str!("anyrail/turnout_joined.json")),
    ("slip_joined", include_str!("anyrail/slip_joined.xml"), include_str!("anyrail/slip_joined.json")),
    ("slip_scrambled", include_str!("anyrail/slip_scrambled.xml"), include_str!("anyrail/slip_scrambled.json")),
    ("slip_cycled", include_str!("anyrail/slip_cycled.xml"), include_str!("anyrail/slip_cycled.json")),
];

/// Same keys, same strings, and numbers within a tolerance. Reports the first difference found
fn compare(path: &str, actual: &Value, expected: &Value) -> Result<(), String> {
    match (actual, expected) {
        (Value::Number(a), Value::Number(e)) => {
            let (a, e) = (a.as_f64().unwrap_or(f64::NAN), e.as_f64().unwrap_or(f64::NAN));
            if (a - e).abs() <= 1e-6 * e.abs().max(1.0) {
                Ok(())
            } else {
                Err(format!("{path}: {a} != {e}"))
            }
        }
        (Value::Array(a), Value::Array(e)) if a.len() == e.len() => a
            .iter()
            .zip(e)
            .enumerate()
            .try_for_each(|(index, (x, y))| compare(&format!("{path}[{index}]"), x, y)),
        (Value::Object(a), Value::Object(e)) if a.len() == e.len() => e.iter().try_for_each(|(key, expected)| {
            let actual = a.get(key).ok_or_else(|| format!("{path}.{key} missing"))?;
            compare(&format!("{path}.{key}"), actual, expected)
        }),
        _ if actual == expected => Ok(()),
        _ => Err(format!("{path}: {actual} != {expected}")),
    }
}

#[test]
fn every_fixture_matches_the_python_importer() {
    for (name, xml, expected_json) in FIXTURES {
        let expected: Value = serde_json::from_str(expected_json).expect("fixture json");
        let actual = import(xml, 263.5).unwrap_or_else(|error| panic!("{name}: {error}"));
        compare(name, &actual, &expected).unwrap_or_else(|error| panic!("{error}"));
    }
}

#[test]
fn bad_input_is_refused_with_the_same_messages() {
    assert_eq!(import("", 263.5).unwrap_err(), "No layout XML supplied");
    assert!(import("<!DOCTYPE x><layout-config/>", 263.5).unwrap_err().contains("DOCTYPE"));
    assert!(import("<layout-config>", 263.5).unwrap_err().starts_with("Not valid XML"));
    assert_eq!(import("<layout-config/>", 263.5).unwrap_err(), "No layout track found in the XML");
    assert_eq!(
        import("<layout-config><layoutturnout ident=\"T\" type=\"RH_TURNOUT\" xcen=\"0\" ycen=\"0\" xb=\"1\" yb=\"0\" xc=\"1\" yc=\"1\" connectaname=\"a\" connectbname=\"b\" connectcname=\"c\"/></layout-config>", 263.5).unwrap_err(),
        "No 90 degree curves in the layout, so the scale cannot be worked out"
    );
    assert!(import(include_str!("anyrail/basic.xml"), 0.0).is_err());
}

#[test]
fn a_three_way_leg_with_no_point_is_refused_not_a_panic() {
    use crate::anyrail::segments::place_three_ways;
    use crate::anyrail::Key;
    use std::collections::HashMap;

    let key = |ident: &str, kind: &str| -> Key { (ident.to_string(), kind.to_string()) };
    let outer: Vec<Key> = (1..=4).map(|n| key("P", &format!("LEG{n}"))).collect();
    // Each leg links to an anchor point the file never defines
    let links: HashMap<Key, Key> = outer.iter().map(|leg| (leg.clone(), key(&leg.1, "POS_POINT"))).collect();
    let raw = HashMap::new();
    let groups = vec![(vec!["T1".to_string(), "T2".to_string()], outer)];
    let message = place_three_ways(&groups, &raw, 1.0, &links).err().expect("the import should be refused");
    assert!(message.contains("does not define"), "{message}");
}

#[test]
fn a_chosen_file_is_read_as_text_and_a_missing_one_says_why() {
    let path = std::env::temp_dir().join(format!("trainlink-import-{}.xml", std::process::id()));
    std::fs::write(&path, "<layout-config/>").unwrap();
    assert_eq!(crate::import_file::read_xml(&path).unwrap(), "<layout-config/>");
    std::fs::remove_file(&path).unwrap();
    let error = crate::import_file::read_xml(&path).unwrap_err();
    assert!(error.starts_with("Could not read the file:"), "{error}");
}

#[test]
fn only_jpeg_png_and_webp_pictures_are_accepted() {
    use std::path::Path;
    assert_eq!(crate::import_file::picture_type(Path::new("Class 08.JPG")), Some("image/jpeg"));
    assert_eq!(crate::import_file::picture_type(Path::new("shunter.webp")), Some("image/webp"));
    assert_eq!(crate::import_file::picture_type(Path::new("notes.txt")), None);
    assert_eq!(crate::import_file::picture_type(Path::new("no-extension")), None);
}

#[test]
fn a_leg_ending_at_an_anchor_that_no_other_track_touches_is_listed() {
    let xml = r#"<layout-config>
<layoutturnout ident="T1" turnoutname="T1" type="RH_TURNOUT" connectaname="A1" connectbname="B1" connectcname="C1" xcen="0" ycen="0" xb="1" yb="0" xc="1" yc="1" continuing="2"/>
<tracksegment ident="S1" connect1name="T1" type1="TURNOUT_A" connect2name="P1" type2="POS_POINT"/>
<tracksegment ident="S2" connect1name="T1" type1="TURNOUT_B" connect2name="P2" type2="POS_POINT"/>
<tracksegment ident="S3" connect1name="T1" type1="TURNOUT_C" connect2name="P3" type2="POS_POINT"/>
<tracksegment ident="S4" connect1name="P2" type1="POS_POINT" connect2name="P4" type2="POS_POINT"/>
<positionablepoint ident="P1" type="END_BUMPER" x="0" y="0"/>
<positionablepoint ident="P2" type="ANCHOR" x="1" y="0"/>
<positionablepoint ident="P3" type="ANCHOR" x="1" y="1"/>
<positionablepoint ident="P4" type="END_BUMPER" x="2" y="0"/>
</layout-config>"#;
    let found = legs_without_track(xml).unwrap();
    assert_eq!(found, vec![serde_json::json!({ "turnout": "T1", "leg": "C", "point": "P3" })]);
}

#[test]
fn the_sample_fixtures_list_only_their_unattached_legs() {
    // These fixtures are geometry tests, so their turnouts stand alone: every leg ends at an anchor nothing else touches
    let turnout = legs_without_track(include_str!("anyrail/turnout_joined.xml")).unwrap();
    assert_eq!(turnout.len(), 2);
    let slip = legs_without_track(include_str!("anyrail/slip_joined.xml")).unwrap();
    assert_eq!(slip.len(), 3);
}

// OMLIF JSON, the newer AnyRail export. These layouts are built by hand from the schema, not from a file AnyRail wrote

use crate::anyrail::geometry::{normalize, turnout_ends, world};
use crate::anyrail::omlif::import as import_omlif;
use crate::library::library;

fn omlif(units: &str, endpoints: Value, parts: Value) -> String {
    json!({ "layout": { "attributes": { "length": 0, "units": units, "version": "1", "width": 0 }, "connections": [], "endpoints": endpoints, "parts": parts, "sections": [] } }).to_string()
}

fn part(id: i64, kind: &str, ends: &[i64], drawing: Value) -> Value {
    json!({ "attributes": { "bridge": false, "hidden": false, "id": id, "type": kind }, "drawing": drawing, "endpoint-ids": ends, "routes": [] })
}

fn end(id: i64, x: f64, y: f64, direction: f64) -> Value {
    json!({ "coord": [x, y], "direction": direction, "id": id })
}

#[test]
fn an_omlif_straight_is_flex_track_of_its_length() {
    let text = omlif("mm", json!([end(1, 100.0, 100.0, 180.0), end(2, 1014.4, 100.0, 0.0)]), json!([part(7, "straight", &[1, 2], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    let pieces = result["track_pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 1, "{result}");
    assert_eq!(pieces[0]["piece_id"], "peco-n-sl-300f");
    assert!((pieces[0]["length"].as_f64().unwrap() - 914.4).abs() < 0.01);
    assert!(pieces[0]["rotation"].as_f64().unwrap().abs() < 0.01, "travel along +x");
    assert_eq!(result["format"], "omlif");
    assert!(result["unplaced"].as_array().unwrap().is_empty());
}

#[test]
fn centimetres_give_the_same_straight_as_millimetres() {
    let text = omlif("cm", json!([end(1, 10.0, 10.0, 180.0), end(2, 101.44, 10.0, 0.0)]), json!([part(1, "straight", &[1, 2], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    assert!((result["track_pieces"][0]["length"].as_f64().unwrap() - 914.4).abs() < 0.01);
}

#[test]
fn unknown_units_are_refused_rather_than_guessed() {
    let text = omlif("furlongs", json!([end(1, 0.0, 0.0, 180.0), end(2, 1.0, 0.0, 0.0)]), json!([part(1, "straight", &[1, 2], Value::Null)]));
    assert!(import_omlif(&text).unwrap_err().contains("Unknown units"));
}

#[test]
fn a_straight_is_placed_from_its_ends_whatever_their_headings_say() {
    // The headings here do not face opposite ways, which the file's own headings do not always do either
    let text = omlif("mm", json!([end(1, 0.0, 0.0, 180.0), end(2, 0.0, 100.0, 180.0)]), json!([part(1, "straight", &[1, 2], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    let piece = &result["track_pieces"][0];
    assert_eq!(piece["piece_id"], "peco-n-sl-300f");
    assert!((piece["rotation"].as_f64().unwrap() - 90.0).abs() < 0.01, "the chord runs along +y");
    assert!((piece["length"].as_f64().unwrap() - 100.0).abs() < 0.01);
}

/// The ends of a turnout placed at `origin` heading `rotation`, as the file would give them: toe, straight, diverging
fn turnout_ends_json(piece_id: &str, origin: (f64, f64), rotation: f64, first_id: i64) -> Vec<Value> {
    let piece = library().get(piece_id).cloned().expect("library turnout");
    let local = turnout_ends(&piece);
    local
        .iter()
        .enumerate()
        .map(|(index, (point, heading))| {
            let placed = world(*point, origin, rotation, false);
            end(first_id + index as i64, placed.0, placed.1, normalize(rotation + heading))
        })
        .collect()
}

#[test]
fn an_omlif_turnout_is_placed_from_its_ends_with_its_hand() {
    let ends = turnout_ends_json("peco-n-sl-u395f", (200.0, 300.0), 90.0, 1);
    let text = omlif("mm", Value::Array(ends), json!([part(3, "turnout-right", &[1, 2, 3], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    let pieces = result["track_pieces"].as_array().unwrap();
    assert_eq!(pieces.len(), 1, "{result}");
    assert_eq!(pieces[0]["piece_id"], "peco-n-sl-u395f");
    assert!((pieces[0]["rotation"].as_f64().unwrap() - 90.0).abs() < 0.01);
    assert!(result["unplaced"].as_array().unwrap().is_empty(), "{result}");
}

#[test]
fn a_turnout_with_no_hand_is_refused_unless_its_drawing_arc_decides_it() {
    let ends = turnout_ends_json("peco-n-sl-u395f", (0.0, 0.0), 0.0, 1);
    // Points alone fit both hands, so the plain turnout is refused
    let plain = omlif("mm", Value::Array(ends.clone()), json!([part(3, "turnout", &[1, 2, 3], Value::Null)]));
    let result = import_omlif(&plain).unwrap();
    assert!(result["track_pieces"].as_array().unwrap().is_empty());
    assert!(result["unplaced"][0]["reason"].as_str().unwrap().contains("either hand"));

    // A drawn arc from the entry to the diverging end says which leg diverges
    let toe = [ends[0]["coord"][0].clone(), ends[0]["coord"][1].clone()];
    let diverge = [ends[2]["coord"][0].clone(), ends[2]["coord"][1].clone()];
    let drawing = json!({ "arcs": [{ "angle": 10.0, "center": [0.0, 0.0], "direction": 0.0, "p1": toe, "p2": diverge, "radius": 457.0, "width": 1.0 }], "lines": [], "beziers": [] });
    let drawn = omlif("mm", Value::Array(ends.clone()), json!([part(3, "turnout", &[1, 2, 3], drawing)]));
    let result = import_omlif(&drawn).unwrap();
    assert_eq!(result["track_pieces"][0]["piece_id"], "peco-n-sl-u395f", "{result}");
    assert_eq!(result["guessed_hand"], json!(["omlif-3"]));
}

#[test]
fn a_turnout_typed_with_a_hand_uses_that_hand() {
    let ends = turnout_ends_json("peco-n-sl-u395f", (0.0, 0.0), 0.0, 1);
    let typed = omlif("mm", Value::Array(ends), json!([part(3, "turnout-right", &[1, 2, 3], Value::Null)]));
    let result = import_omlif(&typed).unwrap();
    assert_eq!(result["track_pieces"][0]["piece_id"], "peco-n-sl-u395f");
    assert!(result["guessed_hand"].as_array().unwrap().is_empty());
}

#[test]
fn an_omlif_quarter_curve_is_flex_track_of_its_chord_with_handles() {
    let radius = 457.0;
    let drawing = json!({ "arcs": [{ "angle": 90.0, "center": [0.0, radius], "direction": 0.0, "p1": [0.0, 0.0], "p2": [radius, radius], "radius": radius, "width": 2.0 }], "lines": [], "beziers": [] });
    let text = omlif("mm", json!([end(1, 0.0, 0.0, 180.0), end(2, radius, radius, 90.0)]), json!([part(5, "curve", &[1, 2], drawing)]));
    let result = import_omlif(&text).unwrap();
    let piece = &result["track_pieces"][0];
    assert_eq!(piece["piece_id"], "peco-n-sl-300f");
    assert!((piece["length"].as_f64().unwrap() - radius * 2f64.sqrt()).abs() < 0.01, "{piece}");
    assert_eq!(piece["handles"].as_array().unwrap().len(), 2);
}

#[test]
fn parts_the_importer_cannot_place_are_reported_and_ends_are_skipped() {
    let text = omlif(
        "mm",
        json!([end(1, 0.0, 0.0, 180.0), end(2, 100.0, 0.0, 0.0), end(3, 200.0, 0.0, 0.0)]),
        json!([part(1, "crossing", &[1, 2], Value::Null), part(2, "buffer", &[2, 3], Value::Null)]),
    );
    let result = import_omlif(&text).unwrap();
    assert_eq!(result["unplaced"][0]["reason"], "crossing is not imported yet");
    assert_eq!(result["skipped"][0]["reason"], "an end of the track, not a piece");
}

#[test]
fn an_omlif_file_is_refused_when_it_is_not_json_of_the_right_shape() {
    assert!(import_omlif("{ not json").unwrap_err().contains("Not a valid AnyRail JSON layout"));
    assert!(import_omlif("{\"layout\": {}}").is_err());
}

#[test]
fn an_omlif_slip_is_placed_from_its_four_ends_like_the_xml_import() {
    let geometry = library().get("peco-n-sl-e390f").cloned().expect("library slip")["slip_geometry"].clone();
    let names = ["track_a_top", "track_b_top", "track_a_bottom", "track_b_bottom"];
    let ends: Vec<Value> = names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let local = (geometry[*name][0].as_f64().unwrap(), geometry[*name][1].as_f64().unwrap());
            let placed = world(local, (100.0, 100.0), 0.0, false);
            end(index as i64 + 1, placed.0, placed.1, 0.0)
        })
        .collect();
    let text = omlif("mm", Value::Array(ends), json!([part(9, "slipswitch", &[1, 2, 3, 4], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    assert_eq!(result["track_pieces"][0]["piece_id"], "peco-n-sl-e390f", "{result}");
    assert!(result["track_pieces"][0]["rotation"].as_f64().unwrap().abs() < 0.01);
    assert!(result["gaps"].as_array().unwrap().is_empty(), "{result}");
}

#[test]
fn an_omlif_three_way_is_calculated_from_the_track_that_enters_and_leaves_it() {
    let library_piece = library().get("peco-n-sl-e399f").cloned().expect("library three-way");
    let local = crate::anyrail::geometry::three_way_ends(&library_piece);
    // The track as the file would give it: the three-way turned by 30 degrees and moved
    let ends: Vec<Value> = local
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let placed = world(*point, (300.0, 200.0), 30.0, false);
            end(index as i64 + 1, placed.0, placed.1, 0.0)
        })
        .collect();
    let text = omlif("mm", Value::Array(ends), json!([part(8, "threeway", &[1, 2, 3, 4], Value::Null)]));
    let result = import_omlif(&text).unwrap();
    let piece = &result["track_pieces"][0];
    assert_eq!(piece["piece_id"], "peco-n-sl-e399f", "{result}");
    assert!((piece["rotation"].as_f64().unwrap() - 30.0).abs() < 0.01, "{piece}");
    assert!(result["gaps"].as_array().unwrap().is_empty(), "{result}");
}

#[test]
fn an_omlif_file_with_a_byte_order_mark_is_still_read_as_json() {
    let text = format!("\u{feff}{}", omlif("mm", json!([end(1, 0.0, 0.0, 180.0), end(2, 100.0, 0.0, 0.0)]), json!([part(1, "straight", &[1, 2], Value::Null)])));
    let result = import_omlif(&text).unwrap();
    assert_eq!(result["track_pieces"].as_array().unwrap().len(), 1);
}

#[test]
fn a_cut_off_omlif_file_says_it_is_incomplete() {
    let full = omlif("mm", json!([end(1, 0.0, 0.0, 180.0), end(2, 100.0, 0.0, 0.0)]), json!([part(1, "straight", &[1, 2], Value::Null)]));
    let cut = &full[..full.len() / 2];
    assert!(import_omlif(cut).unwrap_err().contains("ends early"));
}

#[test]
fn track_that_joins_a_turnout_starts_at_the_turnouts_exact_leg() {
    // The straight's end is 0.6 mm from the turnout's straight leg, as the file's coordinates can be
    let mut ends = turnout_ends_json("peco-n-sl-u395f", (0.0, 0.0), 0.0, 1);
    let leg = (ends[1]["coord"][0].as_f64().unwrap(), ends[1]["coord"][1].as_f64().unwrap());
    ends.push(end(4, leg.0, leg.1 + 0.6, 0.0));
    ends.push(end(5, leg.0 + 500.0, leg.1 + 0.6, 0.0));
    let text = format!(
        "{}",
        json!({
            "layout": {
                "attributes": { "length": 0, "units": "mm", "version": "1", "width": 0 },
                "connections": [[2, 4]],
                "endpoints": ends,
                "parts": [part(3, "turnout-right", &[1, 2, 3], Value::Null), part(4, "straight", &[4, 5], Value::Null)],
                "sections": []
            }
        })
    );
    let result = import_omlif(&text).unwrap();
    let turnout = result["track_pieces"].as_array().unwrap().iter().find(|piece| piece["piece_id"] == "peco-n-sl-u395f").unwrap().clone();
    let straight = result["track_pieces"].as_array().unwrap().iter().find(|piece| piece["piece_id"] == "peco-n-sl-300f").unwrap().clone();
    let piece = library().get("peco-n-sl-u395f").cloned().unwrap();
    let local_straight = turnout_ends(&piece)[1].0;
    let exact = world(local_straight, (turnout["x"].as_f64().unwrap(), turnout["y"].as_f64().unwrap()), turnout["rotation"].as_f64().unwrap(), false);
    let start = (straight["x"].as_f64().unwrap(), straight["y"].as_f64().unwrap());
    assert!(((start.0 - exact.0).powi(2) + (start.1 - exact.1).powi(2)).sqrt() < 0.01, "straight starts at {start:?}, turnout leg is at {exact:?}");
}
