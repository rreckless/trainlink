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

#[test]
fn type_is_one_of_the_known_kinds() {
    let app = app();
    assert_eq!(send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Tram", "dcc_address": 3, "locomotive_type": "tram" }))).0, 201);
    let (status, value) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Boat", "dcc_address": 4, "locomotive_type": "boat" })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "Locomotive type must be one of: steam, diesel, electric, multiple unit, tram, shunt, museum, other");
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 5)));
    let id = created["id"].as_i64().unwrap();
    assert_eq!(send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "locomotive_type": "boat" }))).0, 422);
}

#[test]
fn group_max_speed_and_colour_are_saved_and_can_be_cleared() {
    let app = app();
    let body = json!({
        "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel",
        "group_name": " Freight ", "max_speed": 60, "colour": "#1E40AF", "reverse_direction": true,
    });
    let (status, created) = send(&app, "POST", "/api/locomotives", Some(body));
    assert_eq!(status, 201, "{created}");
    assert_eq!(created["group_name"], "Freight");
    assert_eq!(created["max_speed"], 60);
    assert_eq!(created["colour"], "#1e40af", "stored in lower case");
    assert_eq!(created["reverse_direction"], true);
    let id = created["id"].as_i64().unwrap();

    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "group_name": null, "max_speed": null, "colour": "" })));
    assert_eq!(status, 200, "{value}");
    assert!(value["group_name"].is_null());
    assert!(value["max_speed"].is_null());
    assert!(value["colour"].is_null());
    assert_eq!(value["reverse_direction"], true, "not in this change, so kept");

    let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "reverse_direction": false })));
    assert_eq!(status, 200);
    assert_eq!(value["reverse_direction"], false);
}

#[test]
fn max_speed_and_colour_are_checked() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    for bad in [json!({ "max_speed": 0 }), json!({ "max_speed": 127 })] {
        let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(bad));
        assert_eq!(status, 422, "{value}");
        assert_eq!(detail(&value), "Maximum speed must be a speed step from 1 to 126");
    }
    for bad in ["blue", "#12345", "#1234567", "#gggggg", "123456"] {
        let (status, value) = send(&app, "PATCH", &format!("/api/locomotives/{id}"), Some(json!({ "colour": bad })));
        assert_eq!(status, 422, "{bad}: {value}");
        assert_eq!(detail(&value), "Colour must be a hex colour such as #1e40af");
    }
}

#[test]
fn picture_may_be_up_to_16_mb() {
    let app = app();
    let (_, created) = send(&app, "POST", "/api/locomotives", Some(loco("Class 37", 3)));
    let id = created["id"].as_i64().unwrap();
    let jpeg = |size: usize| {
        let mut data = b"\xff\xd8\xff".to_vec();
        data.resize(size, 0);
        data
    };
    let upload = |data: Vec<u8>| {
        let reply = crate::api::handle(&app, &crate::api::Call {
            method: "POST",
            path: &format!("/api/locomotives/{id}/image"),
            query: "",
            body: &data,
            content_type: "image/jpeg",
        });
        (reply.status, serde_json::from_slice::<serde_json::Value>(&reply.body).unwrap_or_default())
    };
    let (status, _) = upload(jpeg(5 * 1024 * 1024));
    assert_eq!(status, 200, "5 MB is over the old 2 MB limit and must now be kept");
    let (status, value) = upload(jpeg(16 * 1024 * 1024 + 1));
    assert_eq!(status, 413, "{value}");
    assert_eq!(detail(&value), "Picture must be 16 MB or smaller");
}
