"""
Z21 locomotive functions, state replies, and routes, checked against the LAN protocol spec v1.13
(sections 4.1, 4.3.1, and 4.4).
"""

import asyncio

import pytest
from fastapi.testclient import TestClient

from app.api.z21 import get_z21
from app.drivers.z21_driver import Z21Driver
from app.drivers.z21_locos import build_loco_info_request, build_set_function, parse_loco_info
from app.main import app


def test_set_function_on_and_off_match_the_frame_layout():
    # Loco 3, F0 on: E4 F8 00 03 40, XOR 0x5F. Off is TT=00: XOR 0x1F
    assert build_set_function(3, 0, True) == bytes.fromhex("0a004000" "e4f80003405f")
    assert build_set_function(3, 0, False) == bytes.fromhex("0a004000" "e4f800030 01f".replace(" ", ""))


def test_long_address_gets_the_high_bits():
    frame = build_set_function(200, 4, True)
    assert frame[6:8] == bytes([0xC0, 0xC8])


def test_loco_info_request_matches_the_spec():
    assert build_loco_info_request(3) == bytes.fromhex("09004000" "e3f00003" "10")


def test_function_index_out_of_range_is_refused():
    with pytest.raises(ValueError):
        build_set_function(3, 32, True)


def test_parse_loco_info_reports_speed_direction_and_functions():
    # Address 3, 128 steps, step 0x42 forward (speed 65), F0 and F1 on
    reply = bytes([0xEF, 0x00, 0x03, 0x04, 0x80 | 0x42, 0x11, 0x00, 0x00, 0x00])
    assert parse_loco_info(reply) == {"address": 3, "speed": 65, "direction": "forward", "functions": [0, 1]}


def test_parse_loco_info_stop_and_emergency_stop_are_speed_zero():
    assert parse_loco_info(bytes([0xEF, 0x00, 0x03, 0x04, 0x01, 0, 0, 0, 0]))["speed"] == 0
    assert parse_loco_info(bytes([0xEF, 0x00, 0x03, 0x04, 0x00, 0, 0, 0, 0]))["speed"] == 0


def test_parse_loco_info_reverse_and_high_functions():
    # Reverse, step 10 (speed 9). F5 is bit 0 of DB5, F29 is bit 0 of the optional DB8
    reply = bytes([0xEF, 0x00, 0x03, 0x04, 0x0A, 0x00, 0x01, 0x00, 0x00, 0x01])
    info = parse_loco_info(reply)
    assert info["direction"] == "reverse"
    assert info["speed"] == 9
    assert 5 in info["functions"] and 29 in info["functions"]


def test_parse_loco_info_other_speed_formats_have_no_speed():
    # KKK=0 is 14 steps: the speed is not decoded
    assert parse_loco_info(bytes([0xEF, 0x00, 0x03, 0x00, 0x05, 0, 0, 0, 0]))["speed"] is None


def test_parse_loco_info_too_short_is_none():
    assert parse_loco_info(bytes([0xEF, 0x00, 0x03])) is None


class FakeDriver(Z21Driver):
    def __init__(self):
        super().__init__()
        self.sent = []

    async def _send_message(self, message: bytes):
        self.sent.append(message)


def test_loco_reply_updates_driver_state():
    driver = FakeDriver()
    # Full frame: DataLen 0x0E, header 0x40 0x00, then the X-Bus data and XOR
    data = bytes([0xEF, 0x00, 0x03, 0x04, 0x80 | 0x42, 0x11, 0x00, 0x00, 0x00])
    frame = bytes([0x0E, 0x00, 0x40, 0x00]) + data + bytes([0])
    asyncio.run(driver._handle_message(frame))
    assert driver.locos[3] == {"speed": 65, "direction": "forward", "functions": [0, 1]}


@pytest.fixture
def client():
    yield TestClient(app)
    app.dependency_overrides.clear()


def _driver(connected=True, track_on=True) -> FakeDriver:
    driver = FakeDriver()
    driver.connected = connected
    if connected:
        driver.central_state = {"emergency_stop": False, "track_power_on": track_on, "short_circuit": False}
    return driver


def _use(driver):
    async def override():
        return driver

    app.dependency_overrides[get_z21] = override


def test_function_refused_when_not_connected(client):
    _use(_driver(connected=False))
    assert client.post("/api/z21/loco/function", json={"address": 3, "function": 0, "on": True}).status_code == 503


def test_function_refused_when_track_power_is_off(client):
    _use(_driver(track_on=False))
    assert client.post("/api/z21/loco/function", json={"address": 3, "function": 0, "on": True}).status_code == 409


def test_function_sends_the_frame_when_ready(client):
    driver = _driver()
    _use(driver)
    response = client.post("/api/z21/loco/function", json={"address": 3, "function": 0, "on": True})
    assert response.status_code == 200
    assert driver.sent == [build_set_function(3, 0, True)]


def test_function_rejects_bad_input(client):
    _use(_driver())
    assert client.post("/api/z21/loco/function", json={"address": 0, "function": 0, "on": True}).status_code == 422
    assert client.post("/api/z21/loco/function", json={"address": 3, "function": 32, "on": True}).status_code == 422


def test_loco_states_subscribes_and_returns_known_states(client):
    driver = _driver()
    driver.locos = {3: {"speed": 20, "direction": "forward", "functions": [0]}}
    _use(driver)
    body = client.get("/api/z21/locos", params={"addresses": "3,4"}).json()
    assert body["connected"] is True
    assert body["states"] == {"3": {"speed": 20, "direction": "forward", "functions": [0]}, "4": None}
    assert len(driver.sent) == 2


def test_loco_states_rejects_bad_addresses(client):
    assert client.get("/api/z21/locos", params={"addresses": "x"}).status_code == 400
    assert client.get("/api/z21/locos", params={"addresses": "0"}).status_code == 400
