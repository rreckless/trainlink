use std::time::{Duration, Instant};

use serde_json::json;

use super::{app, detail, send};
use crate::z21::commands::{cv_read, cv_write, pom_read_byte, pom_write_bit, pom_write_byte};
use crate::z21::frames::build_frame;
use crate::z21::replies::CentralState;

fn ready(app: &std::sync::Arc<crate::state::AppState>) {
    let mut inner = app.inner.lock().unwrap();
    inner.z21.connected = true;
    inner.z21.central_state = Some(CentralState { emergency_stop: false, track_power_on: true, short_circuit: false });
}

/// A frame from the Z21 with the given X-Header bytes, with its XOR added (LAN header 0x40 0x00)
fn answer(xbus: &[u8]) -> Vec<u8> {
    let xor = xbus.iter().fold(0, |sum, byte| sum ^ byte);
    let mut data = xbus.to_vec();
    data.push(xor);
    build_frame(0x0040, &data)
}

#[test]
fn a_direct_read_is_the_frame_the_spec_gives() {
    // CV 1 is address 0 (spec 6.1): 0x23 0x11, the CV address MSB and LSB, then the XOR byte
    assert_eq!(cv_read(1).unwrap(), vec![0x09, 0x00, 0x40, 0x00, 0x23, 0x11, 0x00, 0x00, 0x32]);
}

#[test]
fn a_direct_write_is_the_frame_the_spec_gives() {
    // CV 29 is address 28 (spec 6.2): 0x24 0x12, the address, then the value
    assert_eq!(cv_write(29, 6).unwrap(), vec![0x0A, 0x00, 0x40, 0x00, 0x24, 0x12, 0x00, 0x1C, 0x06, 0x2C]);
}

#[test]
fn a_pom_byte_write_is_the_frame_the_spec_gives() {
    // Loco 3, CV 29 (spec 6.6): 0xE6 0x30, the loco address, 0xEC plus the CV's MSB, the CV LSB, the value
    assert_eq!(
        pom_write_byte(3, 29, 6).unwrap(),
        vec![0x0C, 0x00, 0x40, 0x00, 0xE6, 0x30, 0x00, 0x03, 0xEC, 0x1C, 0x06, 0x23]
    );
}

#[test]
fn a_pom_bit_write_sets_the_bit_and_its_value_as_the_spec_gives() {
    // Loco 3, CV 29, bit 2 on (spec 6.7): the last byte is 0000 V PPP, with V the value and PPP the bit position
    assert_eq!(
        pom_write_bit(3, 29, 2, true).unwrap(),
        vec![0x0C, 0x00, 0x40, 0x00, 0xE6, 0x30, 0x00, 0x03, 0xE8, 0x1C, 0x0A, 0x2B]
    );
}

#[test]
fn a_long_loco_address_is_sent_as_the_spec_encodes_it() {
    let frame = pom_write_byte(200, 1, 0).unwrap();
    assert_eq!(&frame[6..8], &[0xC0, 0xC8]);
}

#[test]
fn cv_numbers_and_bits_outside_the_range_are_refused() {
    assert!(cv_read(0).is_err());
    assert!(cv_read(1025).is_err());
    assert!(pom_write_bit(3, 29, 8, true).is_err());
}

#[test]
fn the_answers_from_the_z21_finish_a_read_and_a_write() {
    let app = app();
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.programming.start(29, "read", None, Instant::now());
        // Result of a read: 0x64 0x14, the CV's MSB and LSB, the value
        crate::z21::apply(&mut inner, &answer(&[0x64, 0x14, 0x00, 0x1C, 0x06]), Instant::now());
        let last = inner.z21.programming.last.clone().unwrap();
        assert_eq!((last.cv, last.kind, last.value, last.error), (29, "read", Some(6), None));
        assert!(inner.z21.programming.pending.is_none());
    }
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.programming.start(29, "write", Some(7), Instant::now());
        crate::z21::apply(&mut inner, &answer(&[0x64, 0x14, 0x00, 0x1C, 0x07]), Instant::now());
        assert_eq!(inner.z21.programming.last.clone().unwrap().kind, "write");
    }
}

#[test]
fn a_short_circuit_and_a_missing_acknowledgement_are_reported() {
    let app = app();
    let mut inner = app.inner.lock().unwrap();
    inner.z21.programming.start(3, "read", None, Instant::now());
    crate::z21::apply(&mut inner, &answer(&[0x61, 0x12]), Instant::now());
    assert_eq!(inner.z21.programming.last.clone().unwrap().error.unwrap(), "The programming track is short-circuited");

    inner.z21.programming.start(3, "write", Some(1), Instant::now());
    crate::z21::apply(&mut inner, &answer(&[0x61, 0x13]), Instant::now());
    assert_eq!(inner.z21.programming.last.clone().unwrap().error.unwrap(), "The decoder did not acknowledge the command");
}

#[test]
fn status_is_idle_at_first_and_a_read_needs_a_ready_track() {
    let app = app();
    assert_eq!(send(&app, "GET", "/api/programming/status", None).1, json!({ "pending": null, "last": null }));
    assert_eq!(send(&app, "POST", "/api/programming/read", Some(json!({ "cv": 29 }))).0, 503);
}

#[test]
fn a_write_needs_confirmation_and_values_in_range() {
    let app = app();
    ready(&app);
    let (status, value) = send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 29, "value": 6 })));
    assert_eq!(status, 422);
    assert!(detail(&value).starts_with("Confirm the write first"));
    assert_eq!(send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 29, "value": 300, "confirm": true }))).0, 422);
    assert_eq!(send(&app, "POST", "/api/programming/read", Some(json!({ "cv": 0 }))).0, 422);
}

#[test]
fn a_read_that_cannot_be_sent_is_not_left_waiting() {
    let app = app();
    ready(&app);
    // No socket in the tests, so the Z21 cannot take the command
    assert_eq!(send(&app, "POST", "/api/programming/read", Some(json!({ "cv": 29 }))).0, 500);
    assert_eq!(send(&app, "GET", "/api/programming/status", None).1["pending"], json!(null));
}

#[test]
fn one_operation_at_a_time_and_a_silent_decoder_times_out() {
    let app = app();
    ready(&app);
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.programming.start(29, "read", None, Instant::now());
    }
    assert_eq!(send(&app, "POST", "/api/programming/read", Some(json!({ "cv": 29 }))).0, 409);
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.programming.start(29, "read", None, Instant::now() - Duration::from_secs(31));
    }
    let (_, status) = send(&app, "GET", "/api/programming/status", None);
    assert_eq!(status["pending"], json!(null));
    assert_eq!(status["last"]["error"], "No answer from the Z21 in time");
}

#[test]
fn a_pom_write_needs_a_locomotive_on_the_roster() {
    let app = app();
    ready(&app);
    let (status, _) = send(&app, "POST", "/api/programming/pom/write", Some(json!({ "locomotive_id": 999, "cv": 29, "value": 6, "confirm": true })));
    assert_eq!(status, 404);
}

#[test]
fn a_pom_byte_read_is_the_frame_the_spec_gives() {
    // Loco 3, CV 29 (spec 6.8): 0xE6 0x30, the loco address, 0xE4 plus the CV's MSB, the CV LSB, then 0x00
    assert_eq!(
        pom_read_byte(3, 29).unwrap(),
        vec![0x0C, 0x00, 0x40, 0x00, 0xE6, 0x30, 0x00, 0x03, 0xE4, 0x1C, 0x00, 0x2D]
    );
}

#[test]
fn a_pom_read_needs_a_locomotive_and_a_ready_track() {
    let app = app();
    assert_eq!(send(&app, "POST", "/api/programming/pom/read", Some(json!({ "locomotive_id": 1, "cv": 29 }))).0, 404);
    let (status, _) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    assert_eq!(status, 201);
    assert_eq!(send(&app, "POST", "/api/programming/pom/read", Some(json!({ "locomotive_id": 1, "cv": 29 }))).0, 503);
}

#[test]
fn a_profile_limits_which_cvs_a_write_may_change() {
    let app = app();
    ready(&app);
    let (_, profile) = send(&app, "GET", "/api/decoder-profiles", None);
    let profile_id = profile[0]["id"].as_i64().unwrap();
    // CV 7 is read-only in the NMRA profile, CV 40 is not in it at all, and CV 2 takes 0 to 255
    let (status, value) = send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 7, "value": 1, "confirm": true, "profile_id": profile_id })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "CV 7 is read-only in the profile NMRA standard");
    let (status, value) = send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 40, "value": 1, "confirm": true, "profile_id": profile_id })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "CV 40 is not in the profile NMRA standard");
    let (status, value) = send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 1, "value": 200, "confirm": true, "profile_id": profile_id })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "CV 1 takes a value from 1 to 127 in the profile NMRA standard");
    // A value inside the range gets as far as the Z21, which has no socket in the tests
    assert_eq!(send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 1, "value": 3, "confirm": true, "profile_id": profile_id }))).0, 500);
    assert_eq!(send(&app, "POST", "/api/programming/write", Some(json!({ "cv": 1, "value": 3, "confirm": true, "profile_id": 999 }))).0, 404);
}

#[test]
fn a_profile_limits_pom_writes_too() {
    let app = app();
    ready(&app);
    let (_, profile) = send(&app, "GET", "/api/decoder-profiles", None);
    let profile_id = profile[0]["id"].as_i64().unwrap();
    let (_, loco) = send(&app, "POST", "/api/locomotives", Some(json!({ "name": "Class 37", "dcc_address": 3, "locomotive_type": "diesel" })));
    let loco_id = loco["id"].as_i64().unwrap();
    let (status, value) = send(&app, "POST", "/api/programming/pom/write", Some(json!({ "locomotive_id": loco_id, "cv": 8, "value": 1, "confirm": true, "profile_id": profile_id })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "CV 8 is read-only in the profile NMRA standard");
    let (status, value) = send(&app, "POST", "/api/programming/pom/bit", Some(json!({ "locomotive_id": loco_id, "cv": 7, "bit": 0, "on": true, "confirm": true, "profile_id": profile_id })));
    assert_eq!(status, 422);
    assert_eq!(detail(&value), "CV 7 is read-only in the profile NMRA standard");
}
