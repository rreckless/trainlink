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
