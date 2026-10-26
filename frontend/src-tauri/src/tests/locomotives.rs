use serde_json::json;

use super::{app, detail, send};

fn loco(name: &str, address: i64) -> serde_json::Value {
    json!({ "name": name, "dcc_address": address, "locomotive_type": "diesel" })
}

#[test]
fn create_and_get_locomotive_with_defaults() {
    let app = app();
    let (status, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    assert_eq!(status, 201);
    assert_eq!((created["speed"].as_i64(), created["direction"].as_str()), (Some(0), Some("stop")));
    assert_eq!(created["function_labels"].as_array().unwrap().len(), 20);
    assert_eq!(created["function_labels"][0]["label"], "Light");
    assert_eq!(created["function_labels"][1]["label"], "F1");
    assert!(created["image_url"].is_null());
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "GET", &format!("/api/locomotives/{id}"), None).1["name"], "Class 37");
}

#[test]
fn duplicate_name_or_address_is_refused() {
    let app = app();
    send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let (status, value) = send(&app, "POST", "/api/locomotives", Some(loco("Other", 3)));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "DCC address 3 is already used by Class 37 on this command station");
    let (status, value) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 4)));
    assert_eq!(status, 409);
    assert_eq!(detail(&value), "Locomotive name or DCC address already exists");
}

#[test]
fn address_must_be_1_to_9999() {
    let app = app();
    assert_eq!(send(&app, "POST", "/api/locomotives", Some(loco("A", 0))).0, 400);
    assert_eq!(send(&app, "POST", "/api/locomotives", Some(loco("B", 10000))).0, 400);
}

#[test]
fn speed_and_direction_are_not_set_through_the_roster() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "speed": 10 })));
    assert_eq!(status, 400);
    assert_eq!(detail(&value), "Speed and direction are set through /api/z21/loco/speed");
    assert_eq!(send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "name": null }))).0, 400);
}

#[test]
fn function_labels_need_twenty_non_blank_entries() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    let mut labels: Vec<serde_json::Value> = (0..20).map(|index| json!({ "label": format!("F{index}") })).collect();
    labels[1] = json!({ "label": "  Horn  ", "momentary": true });
    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "function_labels": labels })));
    assert_eq!(status, 200);
    assert_eq!(value["function_labels"][1], json!({ "label": "Horn", "momentary": true }));

    let short = vec![json!({ "label": "Light" })];
    assert_eq!(send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "function_labels": short }))).0, 422);
    let mut blank: Vec<serde_json::Value> = (0..20).map(|_| json!({ "label": "x" })).collect();
    blank[0] = json!({ "label": "   " });
    assert_eq!(send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "function_labels": blank }))).0, 422);
}

#[test]
fn picture_must_be_a_real_image_of_the_declared_type() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    let reply = crate::api::handle(&app, &crate::api::Call {
        method: "POST",
        path: &format!("/api/locomotives/{id}/image"),
        query: "",
        body: b"not a picture",
        content_type: "image/png",
    });
    assert_eq!(reply.status, 400);

    let empty = crate::api::handle(&app, &crate::api::Call {
        method: "POST",
        path: &format!("/api/locomotives/{id}/image"),
        query: "",
        body: b"",
        content_type: "image/jpeg",
    });
    assert_eq!(empty.status, 400);
    assert_eq!(empty.body, br#"{"detail":"No picture data was received"}"#);

    let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    let reply = crate::api::handle(&app, &crate::api::Call {
        method: "POST",
        path: &format!("/api/locomotives/{id}/image"),
        query: "",
        body: png,
        content_type: "image/png",
    });
    assert_eq!(reply.status, 200);
    let value: serde_json::Value = serde_json::from_slice(&reply.body).unwrap();
    assert_eq!(value["image_url"], format!("/api/locomotives/{id}/image"));

    let picture = crate::api::handle(&app, &crate::api::Call {
        method: "GET",
        path: &format!("/api/locomotives/{id}/image"),
        query: "",
        body: b"",
        content_type: "",
    });
    assert_eq!((picture.status, picture.content_type.as_str(), picture.body.as_slice()), (200, "image/png", &png[..]));

    assert_eq!(send(&app, "DELETE", &format!("/api/locomotives/{id}/image"), None).1["image_url"], json!(null));
}

#[test]
fn delete_locomotive_and_missing_is_404() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "DELETE", &format!("/api/locomotives/{id}"), None).0, 204);
    assert_eq!(send(&app, "GET", &format!("/api/locomotives/{id}"), None).0, 404);
    assert_eq!(send(&app, "DELETE", "/api/locomotives/999", None).0, 404);
}
