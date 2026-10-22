//! The AnyRail importer checked against the web backend's Python output for the same files

use serde_json::Value;

use crate::anyrail::import;

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
