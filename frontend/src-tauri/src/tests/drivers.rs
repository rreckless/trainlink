//! The driver interface: the registry of station types, and the Z21 driver through the same trait every driver has

use std::net::UdpSocket;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{app, send};
use crate::drivers::{self, Driver};
use crate::z21::{self, commands, features::HardwareInfo, Z21};

#[test]
fn only_station_types_with_a_driver_are_accepted() {
    assert!(drivers::is_supported("z21"));
    assert!(!drivers::is_supported("ecos"));
    let app = app();
    let (status, value) = send(&app, "POST", "/api/command-stations", Some(json!({ "name": "ECoS", "station_type": "ecos", "port": "10.0.0.111:15471" })));
    assert_eq!(status, 422, "{value}");
    assert_eq!(value["detail"], "Station type must be one of: z21");
}

#[test]
fn the_z21_driver_names_its_type_and_its_features_from_its_hardware() {
    let app = app();
    let mut inner = app.inner.lock().unwrap();
    assert_eq!(inner.z21.station_type(), "z21");
    assert!(inner.z21.features().is_empty(), "nothing is known before the Z21 reports its hardware");
    inner.z21.hardware = Some(HardwareInfo { hw_type: 0x0200, firmware: 0x0143 });
    assert_eq!(inner.z21.features(), vec!["pom_read"]);
}

#[test]
fn a_driver_refuses_commands_while_the_station_is_not_connected() {
    let app = app();
    let mut inner = app.inner.lock().unwrap();
    assert!(!inner.z21.connected());
    assert_eq!(Driver::drive(&mut inner.z21, 3, 50, true).unwrap_err(), "Z21 not connected");
    assert_eq!(Driver::emergency_stop(&mut inner.z21).unwrap_err(), "Z21 not connected");
    assert_eq!(Driver::track_power(&mut inner.z21, false).unwrap_err(), "Z21 not connected");
}

#[test]
fn a_driver_refuses_values_the_protocol_cannot_carry() {
    let app = app();
    let mut inner = app.inner.lock().unwrap();
    assert!(Driver::drive(&mut inner.z21, 3, 127, true).is_err(), "speed above 126");
    assert!(Driver::drive(&mut inner.z21, 0, 10, true).is_err(), "address 0");
    assert!(Driver::set_function(&mut inner.z21, 3, 200, true).is_err(), "function above the last one");
}

/// Commands the driver sends through the trait reach the Z21 as the command builders make them
#[test]
fn commands_through_the_driver_reach_the_z21_as_the_command_builders_make_them() {
    let listener = UdpSocket::bind("127.0.0.1:0").expect("bind listener");
    listener.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = app();
    assert!(z21::connect(&app, "127.0.0.1", port));
    {
        let mut inner = app.inner.lock().unwrap();
        Driver::drive(&mut inner.z21, 3, 50, true).expect("drive");
        Driver::set_function(&mut inner.z21, 3, 1, true).expect("function");
        Driver::track_power(&mut inner.z21, false).expect("track power off");
        Driver::emergency_stop(&mut inner.z21).expect("emergency stop");
    }

    let mut received: Vec<Vec<u8>> = Vec::new();
    let mut buffer = [0u8; 1024];
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(length) = listener.recv(&mut buffer) {
            received.push(buffer[..length].to_vec());
        }
    }
    for expected in [
        commands::loco_drive(3, 50, true).unwrap(),
        commands::set_function(3, 1, true).unwrap(),
        commands::track_power_off(),
        commands::emergency_stop(),
    ] {
        assert!(received.contains(&expected), "{expected:02x?} was not sent");
    }
    app.inner.lock().unwrap().z21.disconnect();
}

#[test]
fn a_driver_connects_to_a_station_and_disconnects_from_it() {
    let fake = super::fake_z21::fake_z21();
    let app = app();
    assert!(<Z21 as Driver>::connect(&app, "127.0.0.1", fake.port));
    assert!(app.inner.lock().unwrap().z21.connected());
    assert!(fake.wait_for(&commands::status_request()), "connecting asks the station for its status");

    Driver::disconnect(&mut app.inner.lock().unwrap().z21);
    assert!(!app.inner.lock().unwrap().z21.connected());
}

#[test]
fn a_driver_asks_for_feedback_and_reports_the_inputs_it_has_seen() {
    let fake = super::fake_z21::fake_z21();
    let app = app();
    assert!(<Z21 as Driver>::connect(&app, "127.0.0.1", fake.port));
    {
        let mut inner = app.inner.lock().unwrap();
        inner.z21.feedback.insert((1, 2), true);
        inner.z21.feedback.insert((1, 3), false);
        assert_eq!(
            inner.z21.feedback_inputs(),
            vec![
                drivers::FeedbackInput { module: 1, input: 2, occupied: true },
                drivers::FeedbackInput { module: 1, input: 3, occupied: false },
            ]
        );
        inner.z21.request_feedback();
    }
    for group in commands::RBUS_GROUPS {
        assert!(fake.wait_for(&commands::rbus_getdata(group)), "feedback group {group} is asked for");
    }
    app.inner.lock().unwrap().z21.disconnect();
}
