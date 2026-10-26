use serde_json::json;

use super::{app, detail, send};

fn profile_body(name: &str, cvs: serde_json::Value) -> serde_json::Value {
    json!({ "name": name, "manufacturer": "Zimo", "description": "Sound decoder", "cvs": cvs, "functions": [{ "index": 0, "label": "Lights" }] })
}

#[test]
fn the_nmra_standard_is_there_and_cannot_be_deleted() {
    let app = app();
    let (status, listed) = send(&app, "GET", "/api/decoder-profiles", None);
    assert_eq!(status, 200);
    assert_eq!(listed[0]["name"], "NMRA standard");
    assert_eq!(listed[0]["builtin"], true);
    let id = listed[0]["id"].as_i64().unwrap();
    let (status, value) = send(&app, "DELETE", &format!("/api/decoder-profiles/{id}"), None);
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "The built-in profile cannot be deleted");
    assert_eq!(send(&app, "GET", &format!("/api/decoder-profiles/{id}"), None).0, 200);
}

#[test]
fn a_profile_can_be_imported_read_and_deleted() {
    let app = app();
    let body = profile_body("Zimo MX630", json!([{ "cv": 3, "name": "Acceleration", "min": 0, "max": 255 }, { "cv": 7, "name": "Version", "read_only": true }]));
    let (status, created) = send(&app, "POST", "/api/decoder-profiles", Some(body.clone()));
    assert_eq!(status, 201, "{created}");
    assert_eq!(created["builtin"], false);
    assert_eq!(created["cvs"][1]["read_only"], true);
    assert_eq!(created["cvs"][0]["max"], 255);
    assert_eq!(created["functions"][0]["label"], "Lights");
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "GET", &format!("/api/decoder-profiles/{id}"), None).1["name"], "Zimo MX630");

    assert_eq!(send(&app, "POST", "/api/decoder-profiles", Some(body)).0, 409, "names are unique");
    assert_eq!(send(&app, "DELETE", &format!("/api/decoder-profiles/{id}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/decoder-profiles/{id}"), None).0, 404);
}

#[test]
fn a_profile_with_bad_cvs_is_refused() {
    let app = app();
    let cases = [
        (json!([]), "A profile needs 1 to 256 CVs"),
        (json!([{ "cv": 0, "name": "Nothing" }]), "Each CV needs a number from 1 to 1024"),
        (json!([{ "cv": 3, "name": "A" }, { "cv": 3, "name": "B" }]), "CV 3 is listed twice"),
        (json!([{ "cv": 3, "name": "" }]), "The name of CV 3 is required"),
        (json!([{ "cv": 3, "name": "Acc", "min": 200, "max": 10 }]), "CV 3 needs a range inside 0 to 255, with the minimum first"),
    ];
    for (cvs, message) in cases {
        let (status, value) = send(&app, "POST", "/api/decoder-profiles", Some(profile_body("Bad", cvs)));
        assert_eq!(status, 422, "{value}");
        assert_eq!(detail(&value), message);
    }
    let (status, value) = send(&app, "POST", "/api/decoder-profiles", Some(json!({ "name": "No CVs" })));
    assert_eq!(status, 422, "{value}");
}

#[test]
fn a_locomotive_chooses_a_profile_and_can_drop_it() {
    let app = app();
    let (_, listed) = send(&app, "GET", "/api/decoder-profiles", None);
    let profile_id = listed[0]["id"].as_i64().unwrap();
    let (_, loco) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let loco_id = loco["id"].as_i64().unwrap();
    assert!(loco["decoder_profile_id"].is_null());

    let (status, linked) = send(&app, "PATCH", &format!("/api/locomotives/{loco_id}"), Some(json!({ "decoder_profile_id": profile_id })));
    assert_eq!(status, 200, "{linked}");
    assert_eq!(linked["decoder_profile_id"], profile_id);
    assert_eq!(send(&app, "GET", &format!("/api/locomotives/{loco_id}"), None).1["decoder_profile_id"], profile_id);

    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{loco_id}"), Some(json!({ "decoder_profile_id": 999 })));
    assert_eq!(status, 404, "{value}");

    let (status, cleared) = send(&app, "PATCH", &format!("/api/locomotives/{loco_id}"), Some(json!({ "decoder_profile_id": null })));
    assert_eq!(status, 200, "{cleared}");
    assert!(cleared["decoder_profile_id"].is_null());
}

#[test]
fn deleting_a_profile_unlinks_the_locomotives_that_used_it() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/decoder-profiles", Some(profile_body("Spare", json!([{ "cv": 3, "name": "Acceleration" }]))));
    let profile_id = created["id"].as_i64().unwrap();
    let (_, loco) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let loco_id = loco["id"].as_i64().unwrap();
    send(&app, "PATCH", &format!("/api/locomotives/{loco_id}"), Some(json!({ "decoder_profile_id": profile_id })));
    assert_eq!(send(&app, "DELETE", &format!("/api/decoder-profiles/{profile_id}"), None).0, 204);
    assert!(send(&app, "GET", &format!("/api/locomotives/{loco_id}"), None).1["decoder_profile_id"].is_null());
}
